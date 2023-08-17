use std::fmt::Display;
use std::rc::Rc;

use crate::foreign::ExternError;
use crate::representations::interpreted::ExprInst;

/// Some expectation (usually about the argument types of a function) did not
/// hold.
#[derive(Clone)]
pub struct AssertionError {
  value: ExprInst,
  assertion: &'static str,
}

impl AssertionError {
  /// Construct, upcast and wrap in a Result that never succeeds for easy
  /// short-circuiting
  pub fn fail<T>(
    value: ExprInst,
    assertion: &'static str,
  ) -> Result<T, Rc<dyn ExternError>> {
    return Err(Self { value, assertion }.into_extern());
  }

  /// Construct and upcast to [ExternError]
  pub fn ext(value: ExprInst, assertion: &'static str) -> Rc<dyn ExternError> {
    return Self { value, assertion }.into_extern();
  }
}

impl Display for AssertionError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Error: {:?} is not {}", self.value, self.assertion)
  }
}

impl ExternError for AssertionError {}
