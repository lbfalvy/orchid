use std::fs::read_to_string;
use std::path::PathBuf;

use super::{Loaded, Loader, LoadingError};

pub fn file_loader(proj: PathBuf) -> impl Loader + 'static {
  move |path: &[&str]| {
    let dirpath = proj.join(path.join("/"));
    if dirpath.is_dir() || dirpath.is_symlink() {
      return Ok(Loaded::Namespace(
        dirpath.read_dir()?
          .filter_map(|entr| {
            let ent = entr.ok()?;
            let typ = ent.file_type().ok()?;
            let path = ent.path();
            if typ.is_dir() || typ.is_symlink() {
              Some(ent.file_name().to_string_lossy().into_owned())
            } else if typ.is_file() && path.extension()? == "orc" {
              Some(path.file_stem()?.to_string_lossy().into_owned())
            } else { None }
          })
          .collect()
      ))
    }
    let orcfile = dirpath.with_extension("orc");
    if orcfile.is_file() {
      read_to_string(orcfile).map(Loaded::Module).map_err(LoadingError::from)
    } else {
      let pathstr = dirpath.to_string_lossy().into_owned();
      Err(if dirpath.exists() { LoadingError::UnknownNode(pathstr) }
      else { LoadingError::Missing(pathstr) })
    }
  }
}
