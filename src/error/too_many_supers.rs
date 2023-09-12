use super::ProjectError;
use crate::representations::location::Location;
use crate::{Interner, VName};

/// Error produced when an import path starts with more `super` segments
/// than the current module's absolute path
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TooManySupers {
  /// The offending import path
  pub path: VName,
  /// The faulty import statement
  pub location: Location,
}
impl ProjectError for TooManySupers {
  fn description(&self) -> &str {
    "an import path starts with more `super` segments than the current \
     module's absolute path"
  }
  fn message(&self) -> String {
    format!(
      "path {} contains too many `super` steps.",
      Interner::extern_all(&self.path).join("::"),
    )
  }

  fn one_position(&self) -> Location {
    self.location.clone()
  }
}
