use std::rc::Rc;
use std::sync::Arc;

use crate::intern::Token;
use crate::name::{PathSlice, VPath};
use crate::proj_error::{ErrorSansOrigin, ErrorSansOriginObj};

/// Represents the result of loading code from a string-tree form such
/// as the file system. Cheap to clone.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Loaded {
  /// Conceptually equivalent to a sourcefile
  Code(Arc<String>),
  /// Conceptually equivalent to the list of *.orc files in a folder, without
  /// the extension
  Collection(Arc<Vec<Token<String>>>),
}
impl Loaded {
  /// Is the loaded item source code (not a collection)?
  pub fn is_code(&self) -> bool { matches!(self, Loaded::Code(_)) }
  /// Collect the elements in a collection rreport
  pub fn collection(items: impl IntoIterator<Item = Token<String>>) -> Self {
    Self::Collection(Arc::new(items.into_iter().collect()))
  }
}

/// Returned by any source loading callback
pub type FSResult = Result<Loaded, ErrorSansOriginObj>;

/// Type that indicates the type of an entry without reading the contents
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum FSKind {
  /// Invalid path or read error
  None,
  /// Source code
  Code,
  /// Internal tree node
  Collection,
}

/// Distinguished error for missing code
#[derive(Clone, PartialEq, Eq)]
pub struct CodeNotFound(pub VPath);
impl CodeNotFound {
  /// Instantiate error
  pub fn new(path: VPath) -> Self { Self(path) }
}
impl ErrorSansOrigin for CodeNotFound {
  const DESCRIPTION: &'static str = "No source code for path";
  fn message(&self) -> String { format!("{} not found", self.0) }
}

/// A simplified view of a file system for the purposes of source code loading.
/// This includes the real FS and source code, but also various in-memory
/// formats and other sources for libraries and dependencies.
pub trait VirtFS {
  /// Implementation of [VirtFS::read]
  fn get(&self, path: &[Token<String>], full_path: &PathSlice) -> FSResult;
  /// Discover information about a path without reading it.
  ///
  /// Implement this if your vfs backend can do expensive operations
  fn kind(&self, path: &PathSlice) -> FSKind {
    match self.read(path) {
      Err(_) => FSKind::None,
      Ok(Loaded::Code(_)) => FSKind::Code,
      Ok(Loaded::Collection(_)) => FSKind::Collection,
    }
  }
  /// Convert a path into a human-readable string that is meaningful in the
  /// target context.
  fn display(&self, path: &[Token<String>]) -> Option<String>;
  /// Convert the FS handler into a type-erased version of itself for packing in
  /// a tree.
  fn rc(self) -> Rc<dyn VirtFS>
  where Self: Sized + 'static {
    Rc::new(self)
  }
  /// Read a path, returning either a text file, a directory listing or an
  /// error. Wrapper for [VirtFS::get]
  fn read(&self, path: &PathSlice) -> FSResult { self.get(path, path) }
}

impl VirtFS for &dyn VirtFS {
  fn get(&self, path: &[Token<String>], full_path: &PathSlice) -> FSResult {
    (*self).get(path, full_path)
  }
  fn display(&self, path: &[Token<String>]) -> Option<String> { (*self).display(path) }
}

impl<T: VirtFS + ?Sized> VirtFS for Rc<T> {
  fn get(&self, path: &[Token<String>], full_path: &PathSlice) -> FSResult {
    (**self).get(path, full_path)
  }
  fn display(&self, path: &[Token<String>]) -> Option<String> { (**self).display(path) }
}
