use std::rc::Rc;

use super::ProjectError;
use crate::representations::location::Location;
use crate::{Interner, VName};

/// Error produced for the statement `import *`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImportAll {
  /// The file containing the offending import
  pub offender_file: Rc<Vec<String>>,
  /// The module containing the offending import
  pub offender_mod: Rc<VName>,
}
impl ProjectError for ImportAll {
  fn description(&self) -> &str {
    "a top-level glob import was used"
  }
  fn message(&self, i: &Interner) -> String {
    format!("{} imports *", i.extern_all(&self.offender_mod).join("::"))
  }

  fn one_position(&self, _i: &Interner) -> Location {
    Location::File(self.offender_file.clone())
  }
}
