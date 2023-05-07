use std::rc::Rc;

use crate::foreign::ExternError;
use crate::external::assertion_error::AssertionError;
use crate::representations::interpreted::ExprInst;
use crate::representations::Literal;

pub fn with_lit<T>(x: &ExprInst,
  predicate: impl FnOnce(&Literal) -> Result<T, Rc<dyn ExternError>>
) -> Result<T, Rc<dyn ExternError>> {
  x.with_literal(predicate)
    .map_err(|()| AssertionError::ext(x.clone(), "a literal value"))
    .and_then(|r| r)
}

pub fn with_str<T>(x: &ExprInst,
  predicate: impl FnOnce(&String) -> Result<T, Rc<dyn ExternError>>
) -> Result<T, Rc<dyn ExternError>> {
  with_lit(x, |l| {
    if let Literal::Str(s) = l {predicate(&s)} else {
      AssertionError::fail(x.clone(), "a string")?
    }
  })
}

pub fn with_uint<T>(x: &ExprInst,
  predicate: impl FnOnce(u64) -> Result<T, Rc<dyn ExternError>>
) -> Result<T, Rc<dyn ExternError>> {
  with_lit(x, |l| {
    if let Literal::Uint(u) = l {predicate(*u)} else {
      AssertionError::fail(x.clone(), "an uint")?
    }
  })
}