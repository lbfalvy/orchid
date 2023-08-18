use std::rc::Rc;

use super::project_error::ProjectError;
use crate::representations::location::Location;
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
  fn message(&self, i: &Interner) -> String {
    format!(
      "{} is opened multiple times with different visibilities",
      i.extern_all(&self.namespace).join("::")
    )
  }
  fn one_position(&self, _i: &Interner) -> Location {
    Location::File(self.file.clone())
  }
}
