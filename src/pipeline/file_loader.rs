//! File system implementation of the source loader callback
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{fs, io};

use crate::interner::{Interner, Sym};
use crate::pipeline::error::{
  ErrorPosition, ProjectError, UnexpectedDirectory,
};
use crate::utils::iter::box_once;
use crate::utils::{BoxedIter, Cache};

/// All the data available about a failed source load call
#[derive(Debug)]
pub struct FileLoadingError {
  file: io::Error,
  dir: io::Error,
  path: Vec<String>,
}
impl ProjectError for FileLoadingError {
  fn description(&self) -> &str {
    "Neither a file nor a directory could be read from the requested path"
  }
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    box_once(ErrorPosition::just_file(self.path.clone()))
  }
  fn message(&self) -> String {
    format!("File: {}\nDirectory: {}", self.file, self.dir)
  }
}

/// Represents the result of loading code from a string-tree form such
/// as the file system.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Loaded {
  /// Conceptually equivalent to a sourcefile
  Code(Rc<String>),
  /// Conceptually equivalent to the list of *.orc files in a folder, without
  /// the extension
  Collection(Rc<Vec<String>>),
}
impl Loaded {
  /// Is the loaded item source code (not a collection)?
  pub fn is_code(&self) -> bool {
    matches!(self, Loaded::Code(_))
  }
}

/// Returned by any source loading callback
pub type IOResult = Result<Loaded, Rc<dyn ProjectError>>;

/// Load a file from a path expressed in Rust strings, but relative to
/// a root expressed as an OS Path.
pub fn load_file(root: &Path, path: &[impl AsRef<str>]) -> IOResult {
  let full_path = path.iter().fold(root.to_owned(), |p, s| p.join(s.as_ref()));
  let file_path = full_path.with_extension("orc");
  let file_error = match fs::read_to_string(file_path) {
    Ok(string) => return Ok(Loaded::Code(Rc::new(string))),
    Err(err) => err,
  };
  let dir = match fs::read_dir(&full_path) {
    Ok(dir) => dir,
    Err(dir_error) =>
      return Err(
        FileLoadingError {
          file: file_error,
          dir: dir_error,
          path: path.iter().map(|s| s.as_ref().to_string()).collect(),
        }
        .rc(),
      ),
  };
  let names = dir
    .filter_map(Result::ok)
    .filter_map(|ent| {
      let fname = ent.file_name().into_string().ok()?;
      let ftyp = ent.metadata().ok()?.file_type();
      Some(if ftyp.is_dir() {
        fname
      } else {
        fname.strip_suffix(".or")?.to_string()
      })
    })
    .collect();
  Ok(Loaded::Collection(Rc::new(names)))
}

/// Generates a cached file loader for a directory
pub fn mk_cache(root: PathBuf, i: &Interner) -> Cache<Sym, IOResult> {
  Cache::new(move |token: Sym, _this| -> IOResult {
    let path = i.r(token).iter().map(|t| i.r(*t).as_str()).collect::<Vec<_>>();
    load_file(&root, &path)
  })
}

/// Loads the string contents of a file at the given location.
/// If the path points to a directory, raises an error.
pub fn load_text(
  path: Sym,
  load_file: &impl Fn(Sym) -> IOResult,
  i: &Interner,
) -> Result<Rc<String>, Rc<dyn ProjectError>> {
  if let Loaded::Code(s) = load_file(path)? {
    Ok(s)
  } else {
    Err(
      UnexpectedDirectory {
        path: i.r(path).iter().map(|t| i.r(*t)).cloned().collect(),
      }
      .rc(),
    )
  }
}
