//! Errors thrown by the standard library in lieu of in-language error handling
//! for runtime errors such as missing files.

use std::fmt::Display;

use crate::foreign::error::{ExternError, ExternErrorObj, ExternResult};

/// Some external event prevented the operation from succeeding
#[derive(Clone)]
pub struct RuntimeError {
  message: String,
  operation: &'static str,
}

impl RuntimeError {
  /// Construct, upcast and wrap in a Result that never succeeds for easy
  /// short-circuiting
  pub fn fail<T>(message: String, operation: &'static str) -> ExternResult<T> {
    Err(Self { message, operation }.rc())
  }

  /// Construct and upcast to [ExternError]
  pub fn ext(message: String, operation: &'static str) -> ExternErrorObj {
    Self { message, operation }.rc()
  }
}

impl Display for RuntimeError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Error while {}: {}", self.operation, self.message)
  }
}

impl ExternError for RuntimeError {}
