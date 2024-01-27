use std::rc::Rc;
use std::sync::Arc;

use intern_all::Tok;

use crate::error::{ErrorSansLocation, ErrorSansLocationObj};
use crate::name::{PathSlice, VPath};

/// Represents the result of loading code from a string-tree form such
/// as the file system. Cheap to clone.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Loaded {
  /// Conceptually equivalent to a sourcefile
  Code(Arc<String>),
  /// Conceptually equivalent to the list of *.orc files in a folder, without
  /// the extension
  Collection(Arc<Vec<Tok<String>>>),
}
impl Loaded {
  /// Is the loaded item source code (not a collection)?
  pub fn is_code(&self) -> bool { matches!(self, Loaded::Code(_)) }
  /// Collect the elements in a collection rreport
  pub fn collection(items: impl IntoIterator<Item = Tok<String>>) -> Self {
    Self::Collection(Arc::new(items.into_iter().collect()))
  }
}

/// Returned by any source loading callback
pub type FSResult = Result<Loaded, ErrorSansLocationObj>;

/// Distinguished error for missing code
#[derive(Clone, PartialEq, Eq)]
pub struct CodeNotFound(pub VPath);
impl ErrorSansLocation for CodeNotFound {
  const DESCRIPTION: &'static str = "No source code for path";
  fn message(&self) -> String { format!("{} not found", self.0) }
}

/// A simplified view of a file system for the purposes of source code loading.
/// This includes the real FS and source code, but also various in-memory
/// formats and other sources for libraries and dependencies.
pub trait VirtFS {
  /// Implementation of [VirtFS::read]
  fn get(&self, path: &[Tok<String>], full_path: PathSlice) -> FSResult;
  /// Convert a path into a human-readable string that is meaningful in the
  /// target context.
  fn display(&self, path: &[Tok<String>]) -> Option<String>;
  /// Convert the FS handler into a type-erased version of itself for packing in
  /// a tree.
  fn rc(self) -> Rc<dyn VirtFS>
  where Self: Sized + 'static {
    Rc::new(self)
  }
  /// Read a path, returning either a text file, a directory listing or an
  /// error. Wrapper for [VirtFS::get]
  fn read(&self, path: PathSlice) -> FSResult { self.get(path.0, path) }
}

impl VirtFS for &dyn VirtFS {
  fn get(&self, path: &[Tok<String>], full_path: PathSlice) -> FSResult {
    (*self).get(path, full_path)
  }
  fn display(&self, path: &[Tok<String>]) -> Option<String> {
    (*self).display(path)
  }
}
