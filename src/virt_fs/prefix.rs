use intern_all::Tok;
use itertools::Itertools;

use super::common::CodeNotFound;
use super::VirtFS;
use crate::error::ErrorSansLocation;
use crate::name::{PathSlice, VPath};

/// Modify the prefix of a nested file tree
pub struct PrefixFS {
  remove: VPath,
  add: VPath,
  wrapped: Box<dyn VirtFS>,
}
impl PrefixFS {
  /// Modify the prefix of a file tree
  pub fn new(
    wrapped: impl VirtFS + 'static,
    remove: impl AsRef<str>,
    add: impl AsRef<str>,
  ) -> Self {
    Self {
      wrapped: Box::new(wrapped),
      remove: VPath::parse(remove.as_ref()),
      add: VPath::parse(add.as_ref()),
    }
  }
  fn proc_path(&self, path: &[Tok<String>]) -> Option<Vec<Tok<String>>> {
    let path = path.strip_prefix(&self.remove[..])?;
    Some(self.add.0.iter().chain(path).cloned().collect_vec())
  }
}
impl VirtFS for PrefixFS {
  fn get(&self, path: &[Tok<String>], full_path: PathSlice) -> super::FSResult {
    let path = (self.proc_path(path))
      .ok_or_else(|| CodeNotFound(full_path.to_vpath()).pack())?;
    self.wrapped.get(&path, full_path)
  }
  fn display(&self, path: &[Tok<String>]) -> Option<String> {
    self.wrapped.display(&self.proc_path(path)?)
  }
}
