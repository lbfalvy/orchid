use super::{ErrorPosition, ProjectError};
use crate::utils::iter::box_once;
use crate::utils::BoxedIter;

/// Error produced when an import refers to a nonexistent module
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModuleNotFound {
  /// The file containing the invalid import
  pub file: Vec<String>,
  /// The invalid import path
  pub subpath: Vec<String>,
}
impl ProjectError for ModuleNotFound {
  fn description(&self) -> &str {
    "an import refers to a nonexistent module"
  }
  fn message(&self) -> String {
    format!(
      "module {} in {} was not found",
      self.subpath.join("::"),
      self.file.join("/"),
    )
  }
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    box_once(ErrorPosition::just_file(self.file.clone()))
  }
}
