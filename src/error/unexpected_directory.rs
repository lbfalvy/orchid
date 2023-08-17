use super::{ErrorPosition, ProjectError};
use crate::utils::iter::box_once;
use crate::utils::BoxedIter;
use crate::{Interner, VName};

/// Produced when a stage that deals specifically with code encounters
/// a path that refers to a directory
#[derive(Debug)]
pub struct UnexpectedDirectory {
  /// Path to the offending collection
  pub path: VName,
}
impl ProjectError for UnexpectedDirectory {
  fn description(&self) -> &str {
    "A stage that deals specifically with code encountered a path that refers \
     to a directory"
  }
  fn positions(&self, i: &Interner) -> BoxedIter<ErrorPosition> {
    box_once(ErrorPosition::just_file(i.extern_all(&self.path)))
  }
  fn message(&self, i: &Interner) -> String {
    format!(
      "{} was expected to be a file but a directory was found",
      i.extern_all(&self.path).join("/")
    )
  }
}
