use crate::utils::{BoxedIter, iter::box_once};

use super::ErrorPosition;
use super::ProjectError;

/// Produced when a stage that deals specifically with code encounters
/// a path that refers to a directory
#[derive(Debug)]
pub struct UnexpectedDirectory {
  pub path: Vec<String>
}
impl ProjectError for UnexpectedDirectory {
  fn description(&self) -> &str {
      "A stage that deals specifically with code encountered a path \
      that refers to a directory"
  }
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    box_once(ErrorPosition::just_file(self.path.clone()))
  }
  fn message(&self) -> String {
    format!(
      "{} was expected to be a file but a directory was found",
      self.path.join("/")
    )
  }
}