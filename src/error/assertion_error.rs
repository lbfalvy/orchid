use std::fmt::Display;
use std::sync::Arc;

use crate::foreign::{ExternError, XfnResult};
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
  pub fn fail<T>(location: Location, message: &'static str) -> XfnResult<T> {
    Err(Self::ext(location, message))
  }

  /// Construct and upcast to [ExternError]
  pub fn ext(
    location: Location,
    message: &'static str,
  ) -> Arc<dyn ExternError> {
    Self { location, message }.into_extern()
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
