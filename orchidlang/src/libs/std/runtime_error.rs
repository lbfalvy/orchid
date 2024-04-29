//! Errors thrown by the standard library in lieu of in-language error handling
//! for runtime errors such as missing files.

use std::fmt;

use crate::foreign::error::{RTError, RTErrorObj, RTResult};

/// Some external event prevented the operation from succeeding
#[derive(Clone)]
pub struct RuntimeError {
  message: String,
  operation: &'static str,
}

impl RuntimeError {
  /// Construct, upcast and wrap in a Result that never succeeds for easy
  /// short-circuiting
  pub fn fail<T>(message: String, operation: &'static str) -> RTResult<T> {
    Err(Self { message, operation }.pack())
  }

  /// Construct and upcast to [RTErrorObj]
  pub fn ext(message: String, operation: &'static str) -> RTErrorObj {
    Self { message, operation }.pack()
  }
}

impl fmt::Display for RuntimeError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Error while {}: {}", self.operation, self.message)
  }
}

impl RTError for RuntimeError {}
