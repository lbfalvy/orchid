use std::rc::Rc;

use crate::foreign::ExternError;
use crate::external::assertion_error::AssertionError;
use crate::representations::{interpreted::Clause, Literal};

pub fn cls2str(c: &Clause) -> Result<&String, Rc<dyn ExternError>> {
  let literal: &Literal = c.try_into()
    .map_err(|_| AssertionError::ext(c.clone(), "a literal value"))?;
  if let Literal::Str(s) = literal {Ok(s)} else {
    AssertionError::fail(c.clone(), "a string")?
  }
}