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
  pub file: VName,
}
impl ProjectError for VisibilityMismatch {
  fn description(&self) -> &str {
    "Some occurences of a namespace are exported but others are not"
  }
  fn message(&self) -> String {
    format!(
      "{} is opened multiple times with different visibilities",
      Interner::extern_all(&self.namespace).join("::")
    )
  }
  fn one_position(&self) -> Location {
    Location::File(Rc::new(self.file.clone()))
  }
}
