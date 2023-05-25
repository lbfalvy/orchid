use std::rc::Rc;

use super::project_error::{ErrorPosition, ProjectError};
use crate::representations::location::Location;
use crate::utils::iter::box_once;
use crate::utils::BoxedIter;

#[derive(Debug)]
pub struct VisibilityMismatch {
  pub namespace: Vec<String>,
  pub file: Rc<Vec<String>>,
}
impl ProjectError for VisibilityMismatch {
  fn description(&self) -> &str {
    "Some occurences of a namespace are exported but others are not"
  }
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    box_once(ErrorPosition {
      location: Location::File(self.file.clone()),
      message: Some(format!(
        "{} is opened multiple times with different visibilities",
        self.namespace.join("::")
      )),
    })
  }
}
