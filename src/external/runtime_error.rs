use std::{rc::Rc, fmt::Display};

use crate::foreign::ExternError;

#[derive(Clone)]
pub struct RuntimeError {
  message: String,
  operation: &'static str,
}

impl RuntimeError {
  pub fn fail(message: String, operation: &'static str) -> Result<!, Rc<dyn ExternError>> {
    return Err(Self { message, operation }.into_extern())
  }

  pub fn ext(message: String, operation: &'static str) -> Rc<dyn ExternError> {
    return Self { message, operation }.into_extern()
  }
}

impl Display for RuntimeError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Error while {}: {}", self.operation, self.message)
  }
}

impl ExternError for RuntimeError{}