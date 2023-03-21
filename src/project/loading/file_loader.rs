use std::fs::read_to_string;
use std::path::PathBuf;

use lasso::Spur;

use crate::representations::sourcefile::FileEntry;

use super::{Loaded, Loader, LoadingError};

pub fn file_loader<'a, F>(
  proj: PathBuf,
  intern: &'a F
) -> impl Loader + 'a
where F: Fn(&str) -> Spur + 'a {
  move |path: &[&str]| {
    let dirpath = proj.join(path.join("/"));
    if dirpath.is_dir() || dirpath.is_symlink() {
      return Ok(Loaded::AST(
        dirpath.read_dir()?
          .filter_map(|entr| {
            let ent = entr.ok()?;
            let typ = ent.file_type().ok()?;
            let path = ent.path();
            if typ.is_dir() || typ.is_symlink() {
              let name = ent.file_name();
              let spur = intern(name.to_string_lossy().as_ref());
              Some(FileEntry::LazyModule(spur))
            } else if typ.is_file() && path.extension()? == "orc" {
              let name = path.file_stem().expect("extension tested above");
              let spur = intern(name.to_string_lossy().as_ref());
              Some(FileEntry::LazyModule(spur))
            } else { None }
          })
          .collect()
      ))
    }
    let orcfile = dirpath.with_extension("orc");
    if orcfile.is_file() {
      read_to_string(orcfile).map(Loaded::Source).map_err(LoadingError::from)
    } else {
      let pathstr = dirpath.to_string_lossy().into_owned();
      Err(if dirpath.exists() { LoadingError::UnknownNode(pathstr) }
      else { LoadingError::Missing(pathstr) })
    }
  }
}
