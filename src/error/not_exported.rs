use std::rc::Rc;

use super::{ErrorPosition, ProjectError};
use crate::representations::location::Location;
use crate::utils::BoxedIter;
use crate::{Interner, VName};

/// An import refers to a symbol which exists but is not exported.
#[derive(Debug)]
pub struct NotExported {
  /// The containing file - files are always exported
  pub file: VName,
  /// The path leading to the unexported module
  pub subpath: VName,
  /// The offending file
  pub referrer_file: VName,
  /// The module containing the offending import
  pub referrer_subpath: VName,
}
impl ProjectError for NotExported {
  fn description(&self) -> &str {
    "An import refers to a symbol that exists but isn't exported"
  }
  fn positions(&self, i: &Interner) -> BoxedIter<ErrorPosition> {
    Box::new(
      [
        ErrorPosition {
          location: Location::File(Rc::new(i.extern_all(&self.file))),
          message: Some(format!(
            "{} isn't exported",
            i.extern_all(&self.subpath).join("::")
          )),
        },
        ErrorPosition {
          location: Location::File(Rc::new(i.extern_all(&self.referrer_file))),
          message: Some(format!(
            "{} cannot see this symbol",
            i.extern_all(&self.referrer_subpath).join("::")
          )),
        },
      ]
      .into_iter(),
    )
  }
}
