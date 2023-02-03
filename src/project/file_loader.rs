use std::io;
use std::rc::Rc;
use std::fs::read_to_string;
use std::path::PathBuf;

use mappable_rc::Mrc;

use super::loaded::Loaded;

#[derive(Clone, Debug)]
pub enum LoadingError {
  IOErr(Rc<io::Error>),
  UnknownNode(String),
  Missing(String)
}

impl From<io::Error> for LoadingError {
  fn from(inner: io::Error) -> Self {
    LoadingError::IOErr(Rc::new(inner))
  }
}

pub fn file_loader(proj: PathBuf) -> impl FnMut(Mrc<[String]>) -> Result<Loaded, LoadingError> + 'static {
  move |path| {
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
