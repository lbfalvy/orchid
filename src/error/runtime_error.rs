use std::fmt::Display;
use std::sync::Arc;

use crate::foreign::{ExternError, XfnResult};

/// Some external event prevented the operation from succeeding
#[derive(Clone)]
pub struct RuntimeError {
  message: String,
  operation: &'static str,
}

impl RuntimeError {
  /// Construct, upcast and wrap in a Result that never succeeds for easy
  /// short-circuiting
  pub fn fail<T>(message: String, operation: &'static str) -> XfnResult<T> {
    Err(Self { message, operation }.into_extern())
  }

  /// Construct and upcast to [ExternError]
  pub fn ext(
    message: String,
    operation: &'static str,
  ) -> Arc<dyn ExternError> {
    Self { message, operation }.into_extern()
  }
}

impl Display for RuntimeError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Error while {}: {}", self.operation, self.message)
  }
}

impl ExternError for RuntimeError {}
