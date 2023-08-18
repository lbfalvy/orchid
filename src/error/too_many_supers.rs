use std::rc::Rc;

use super::ProjectError;
use crate::representations::location::Location;
use crate::{Interner, VName};

/// Error produced when an import path starts with more `super` segments
/// than the current module's absolute path
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TooManySupers {
  /// The offending import path
  pub path: VName,
  /// The file containing the offending import
  pub offender_file: VName,
  /// The module containing the offending import
  pub offender_mod: VName,
}
impl ProjectError for TooManySupers {
  fn description(&self) -> &str {
    "an import path starts with more `super` segments than the current \
     module's absolute path"
  }
  fn message(&self, i: &Interner) -> String {
    format!(
      "path {} in {} contains too many `super` steps.",
      i.extern_all(&self.path).join("::"),
      i.extern_all(&self.offender_mod).join("::")
    )
  }

  fn one_position(&self, i: &Interner) -> Location {
    Location::File(Rc::new(i.extern_all(&self.offender_file)))
  }
}
