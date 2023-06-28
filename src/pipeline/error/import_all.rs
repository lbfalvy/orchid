use std::rc::Rc;

use super::{ErrorPosition, ProjectError};
use crate::representations::location::Location;
use crate::utils::iter::box_once;
use crate::utils::BoxedIter;

/// Error produced for the statement `import *`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImportAll {
  /// The file containing the offending import
  pub offender_file: Vec<String>,
  /// The module containing the offending import
  pub offender_mod: Vec<String>,
}
impl ProjectError for ImportAll {
  fn description(&self) -> &str {
    "a top-level glob import was used"
  }
  fn message(&self) -> String {
    format!("{} imports *", self.offender_mod.join("::"))
  }

  fn positions(&self) -> BoxedIter<ErrorPosition> {
    box_once(ErrorPosition {
      location: Location::File(Rc::new(self.offender_file.clone())),
      message: Some(format!("{} imports *", self.offender_mod.join("::"))),
    })
  }
}
