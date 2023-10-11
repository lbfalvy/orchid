use std::sync::Arc;

use itertools::Itertools;

use super::ProjectError;
use crate::representations::location::Location;
use crate::VName;

/// Error produced for the statement `import *`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImportAll {
  /// The file containing the offending import
  pub offender_file: Arc<VName>,
  /// The module containing the offending import
  pub offender_mod: Arc<VName>,
}
impl ProjectError for ImportAll {
  fn description(&self) -> &str { "a top-level glob import was used" }
  fn message(&self) -> String {
    format!("{} imports *", self.offender_mod.iter().join("::"))
  }

  fn one_position(&self) -> Location {
    Location::File(self.offender_file.clone())
  }
}
