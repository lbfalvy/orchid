use std::rc::Rc;

use crate::{utils::{BoxedIter, iter::box_once}, representations::location::Location};

use super::{ProjectError, ErrorPosition};

/// Error produced when an import path starts with more `super` segments
/// than the current module's absolute path
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TooManySupers {
  pub path: Vec<String>,
  pub offender_file: Vec<String>,
  pub offender_mod: Vec<String>
}
impl ProjectError for TooManySupers {
  fn description(&self) -> &str {
      "an import path starts with more `super` segments than \
      the current module's absolute path"
  }
  fn message(&self) -> String {
    format!(
      "path {} in {} contains too many `super` steps.",
      self.path.join("::"),
      self.offender_mod.join("::")
    )
  }

  fn positions(&self) -> BoxedIter<ErrorPosition> {
    box_once(ErrorPosition {
      location: Location::File(Rc::new(self.offender_file.clone())),
      message: Some(format!(
        "path {} in {} contains too many `super` steps.",
        self.path.join("::"),
        self.offender_mod.join("::")
      ))
    })
  }
}