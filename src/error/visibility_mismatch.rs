use std::rc::Rc;

use super::project_error::{ErrorPosition, ProjectError};
use crate::representations::location::Location;
use crate::utils::iter::box_once;
use crate::utils::BoxedIter;
use crate::{Interner, VName};

/// Multiple occurences of the same namespace with different visibility
#[derive(Debug)]
pub struct VisibilityMismatch {
  /// The namespace with ambiguous visibility
  pub namespace: VName,
  /// The file containing the namespace
  pub file: Rc<Vec<String>>,
}
impl ProjectError for VisibilityMismatch {
  fn description(&self) -> &str {
    "Some occurences of a namespace are exported but others are not"
  }
  fn positions(&self, i: &Interner) -> BoxedIter<ErrorPosition> {
    box_once(ErrorPosition {
      location: Location::File(self.file.clone()),
      message: Some(format!(
        "{} is opened multiple times with different visibilities",
        i.extern_all(&self.namespace).join("::")
      )),
    })
  }
}
