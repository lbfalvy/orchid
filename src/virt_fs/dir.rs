use std::cell::RefCell;
use std::fs::File;
use std::io;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use hashbrown::HashMap;
use intern_all::{i, Tok};

use super::common::CodeNotFound;
use super::{FSResult, Loaded, VirtFS};
use crate::error::{ErrorSansOrigin, ErrorSansOriginObj};
use crate::name::PathSlice;

#[derive(Clone)]
struct OpenError {
  file: Arc<Mutex<io::Error>>,
  dir: Arc<Mutex<io::Error>>,
}
impl OpenError {
  pub fn wrap(file: io::Error, dir: io::Error) -> ErrorSansOriginObj {
    Self { dir: Arc::new(Mutex::new(dir)), file: Arc::new(Mutex::new(file)) }.pack()
  }
}
impl ErrorSansOrigin for OpenError {
  const DESCRIPTION: &'static str = "A file system error occurred";
  fn message(&self) -> String {
    let Self { dir, file } = self;
    format!(
      "File system errors other than not found occurred\n\
      as a file: {}\nas a directory: {}",
      file.lock().unwrap(),
      dir.lock().unwrap()
    )
  }
}

#[derive(Clone)]
struct IOError(Arc<Mutex<io::Error>>);
impl IOError {
  pub fn wrap(inner: io::Error) -> ErrorSansOriginObj { Self(Arc::new(Mutex::new(inner))).pack() }
}
impl ErrorSansOrigin for IOError {
  const DESCRIPTION: &'static str = "an I/O error occured";
  fn message(&self) -> String { format!("File read error: {}", self.0.lock().unwrap()) }
}

#[derive(Clone)]
struct NotUtf8(PathBuf);
impl NotUtf8 {
  pub fn wrap(path: &Path) -> ErrorSansOriginObj { Self(path.to_owned()).pack() }
}
impl ErrorSansOrigin for NotUtf8 {
  const DESCRIPTION: &'static str = "Source files must be UTF-8";
  fn message(&self) -> String {
    format!("{} is a source file but contains invalid UTF-8", self.0.display())
  }
}

/// A real file system directory linked into the virtual FS
pub struct DirNode {
  cached: RefCell<HashMap<PathBuf, FSResult>>,
  root: PathBuf,
  suffix: &'static str,
}
impl DirNode {
  /// Reference a real file system directory in the virtual FS
  pub fn new(root: PathBuf, suffix: &'static str) -> Self {
    assert!(suffix.starts_with('.'), "Extension must begin with .");
    Self { cached: RefCell::default(), root, suffix }
  }

  fn ext(&self) -> &str { self.suffix.strip_prefix('.').expect("Checked in constructor") }

  fn load_file(&self, fpath: &Path, orig_path: &PathSlice) -> FSResult {
    match fpath.read_dir() {
      Err(dir_e) => {
        let fpath = fpath.with_extension(self.ext());
        let mut file =
          File::open(&fpath).map_err(|file_e| match (dir_e.kind(), file_e.kind()) {
            (ErrorKind::NotFound, ErrorKind::NotFound) =>
              CodeNotFound::new(orig_path.to_vpath()).pack(),
            _ => OpenError::wrap(file_e, dir_e),
          })?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).map_err(IOError::wrap)?;
        let text = String::from_utf8(buf).map_err(|_| NotUtf8::wrap(&fpath))?;
        Ok(Loaded::Code(Arc::new(text)))
      },
      Ok(dir) => Ok(Loaded::collection(dir.filter_map(|ent_r| {
        let ent = ent_r.ok()?;
        let name = ent.file_name().into_string().ok()?;
        match ent.metadata().ok()?.is_dir() {
          false => Some(i(name.strip_suffix(self.suffix)?)),
          true => Some(i(&name)),
        }
      }))),
    }
  }

  fn mk_pathbuf(&self, path: &[Tok<String>]) -> PathBuf {
    let mut fpath = self.root.clone();
    path.iter().for_each(|seg| fpath.push(seg.as_str()));
    fpath
  }
}
impl VirtFS for DirNode {
  fn get(&self, path: &[Tok<String>], full_path: &PathSlice) -> FSResult {
    let fpath = self.mk_pathbuf(path);
    let mut binding = self.cached.borrow_mut();
    let (_, res) = (binding.raw_entry_mut().from_key(&fpath))
      .or_insert_with(|| (fpath.clone(), self.load_file(&fpath, full_path)));
    res.clone()
  }

  fn display(&self, path: &[Tok<String>]) -> Option<String> {
    let pathbuf = self.mk_pathbuf(path).with_extension(self.ext());
    Some(pathbuf.to_string_lossy().to_string())
  }
}
