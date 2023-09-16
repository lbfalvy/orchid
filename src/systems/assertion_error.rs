use std::fmt::Display;
use std::rc::Rc;

use crate::foreign::ExternError;
use crate::Location;

/// Some expectation (usually about the argument types of a function) did not
/// hold.
#[derive(Clone)]
pub struct AssertionError {
  location: Location,
  message: &'static str,
}

impl AssertionError {
  /// Construct, upcast and wrap in a Result that never succeeds for easy
  /// short-circuiting
  pub fn fail<T>(
    location: Location,
    message: &'static str,
  ) -> Result<T, Rc<dyn ExternError>> {
    return Err(Self { location, message }.into_extern());
  }

  /// Construct and upcast to [ExternError]
  pub fn ext(location: Location, message: &'static str) -> Rc<dyn ExternError> {
    return Self { location, message }.into_extern();
  }
}

impl Display for AssertionError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Error: expected {}", self.message)?;
    if self.location != Location::Unknown {
      write!(f, " at {}", self.location)?;
    }
    Ok(())
  }
}

impl ExternError for AssertionError {}
