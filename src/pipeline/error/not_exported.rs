use std::rc::Rc;

use super::{ErrorPosition, ProjectError};
use crate::representations::location::Location;
use crate::utils::BoxedIter;

#[derive(Debug)]
pub struct NotExported {
  pub file: Vec<String>,
  pub subpath: Vec<String>,
  pub referrer_file: Vec<String>,
  pub referrer_subpath: Vec<String>,
}
impl ProjectError for NotExported {
  fn description(&self) -> &str {
    "An import refers to a symbol that exists but isn't exported"
  }
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    Box::new(
      [
        ErrorPosition {
          location: Location::File(Rc::new(self.file.clone())),
          message: Some(format!("{} isn't exported", self.subpath.join("::"))),
        },
        ErrorPosition {
          location: Location::File(Rc::new(self.referrer_file.clone())),
          message: Some(format!(
            "{} cannot see this symbol",
            self.referrer_subpath.join("::")
          )),
        },
      ]
      .into_iter(),
    )
  }
}
