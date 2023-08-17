use std::fmt::Display;
use std::rc::Rc;

use crate::foreign::ExternError;

/// Some external event prevented the operation from succeeding
#[derive(Clone)]
pub struct RuntimeError {
  message: String,
  operation: &'static str,
}

impl RuntimeError {
  /// Construct, upcast and wrap in a Result that never succeeds for easy
  /// short-circuiting
  pub fn fail<T>(
    message: String,
    operation: &'static str,
  ) -> Result<T, Rc<dyn ExternError>> {
    return Err(Self { message, operation }.into_extern());
  }

  /// Construct and upcast to [ExternError]
  pub fn ext(message: String, operation: &'static str) -> Rc<dyn ExternError> {
    return Self { message, operation }.into_extern();
  }
}

impl Display for RuntimeError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Error while {}: {}", self.operation, self.message)
  }
}

impl ExternError for RuntimeError {}
