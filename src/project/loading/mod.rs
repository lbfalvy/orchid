mod file_loader;
mod ext_loader;
mod string_loader;
mod map_loader;
mod extlib_loader;
mod prefix_loader;

pub use file_loader::file_loader;
pub use ext_loader::ext_loader;
pub use extlib_loader::extlib_loader;
pub use string_loader::string_loader;
pub use map_loader::map_loader;
pub use prefix_loader::prefix_loader;

use std::{rc::Rc, io};

use crate::representations::sourcefile::FileEntry;

#[derive(Clone, Debug)]
pub enum LoadingError {
  /// An IO operation has failed (i.e. no read permission)
  IOErr(Rc<io::Error>),
  /// The leaf does not exist
  UnknownNode(String),
  /// The leaf and at least the immediately containing namespace don't exist
  Missing(String)
}

impl From<io::Error> for LoadingError {
  fn from(inner: io::Error) -> Self {
    LoadingError::IOErr(Rc::new(inner))
  }
}

#[derive(Clone)]
pub enum Loaded {
  Source(String),
  AST(Vec<FileEntry>)
}

pub trait Loader {
  fn load<'s, 'a>(&'s mut self, path: &'a [&'a str]) -> Result<Loaded, LoadingError>;
  fn boxed<'a>(self) -> Box<dyn 'a + Loader> where Self: 'a + Sized {
    Box::new(self)
  }
}

impl<T> Loader for T where T: for<'a> FnMut(&'a [&'a str]) -> Result<Loaded, LoadingError> {
  fn load(&mut self, path: &[&str]) -> Result<Loaded, LoadingError> {
    (self)(path)
  }
}

impl Loader for Box<dyn Loader> {
  fn load<'s, 'a>(&'s mut self, path: &'a [&'a str]) -> Result<Loaded, LoadingError> {
    self.as_mut().load(path)
  }
} 