use std::rc::Rc;
use std::fmt::Display;

use crate::foreign::ExternError;
use crate::representations::interpreted::Clause;


#[derive(Clone)]
pub struct AssertionError{
  pub value: Clause,
  pub assertion: &'static str,
}

impl AssertionError {
  pub fn fail(value: Clause, assertion: &'static str) -> Result<!, Rc<dyn ExternError>> {
    return Err(Self { value, assertion }.into_extern())
  }

  pub fn into_extern(self) -> Rc<dyn ExternError> {
    Rc::new(self)
  }
}

impl Display for AssertionError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Error: {:?} is not {}", self.value, self.assertion)
  }
}

impl ExternError for AssertionError{}