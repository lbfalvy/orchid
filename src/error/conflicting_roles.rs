use itertools::Itertools;

use super::{ErrorPosition, ProjectError};
use crate::utils::BoxedIter;
use crate::{Location, VName};

/// Error raised if the same name ends up assigned to more than one thing.
/// A name in Orchid has exactly one meaning, either a value or a module.
pub struct ConflictingRoles {
  /// Name assigned to multiple things
  pub name: VName,
  /// Location of at least two occurrences
  pub locations: Vec<Location>,
}
impl ProjectError for ConflictingRoles {
  fn description(&self) -> &str {
    "The same name is assigned multiple times to conflicting items"
  }
  fn message(&self) -> String {
    format!(
      "{} has multiple conflicting meanings",
      self.name.iter().map(|t| t.as_str()).join("::")
    )
  }
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    Box::new(
      (self.locations.iter())
        .map(|l| ErrorPosition { location: l.clone(), message: None }),
    )
  }
}
