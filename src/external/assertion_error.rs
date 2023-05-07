use std::rc::Rc;
use std::fmt::Display;

use crate::foreign::ExternError;
use crate::representations::interpreted::ExprInst;


#[derive(Clone)]
pub struct AssertionError{
  pub value: ExprInst,
  pub assertion: &'static str,
}

impl AssertionError {
  pub fn fail(value: ExprInst, assertion: &'static str) -> Result<!, Rc<dyn ExternError>> {
    return Err(Self { value, assertion }.into_extern())
  }

  pub fn ext(value: ExprInst, assertion: &'static str) -> Rc<dyn ExternError> {
    return Self { value, assertion }.into_extern()
  }
}

impl Display for AssertionError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Error: {:?} is not {}", self.value, self.assertion)
  }
}

impl ExternError for AssertionError{}