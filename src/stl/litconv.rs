use std::rc::Rc;

use super::assertion_error::AssertionError;
use crate::foreign::ExternError;
use crate::representations::interpreted::ExprInst;
use crate::representations::Literal;

/// Tries to cast the [ExprInst] as a [Literal], calls the provided function on
/// it if successful. Returns a generic [AssertionError] if not.
pub fn with_lit<T>(
  x: &ExprInst,
  predicate: impl FnOnce(&Literal) -> Result<T, Rc<dyn ExternError>>,
) -> Result<T, Rc<dyn ExternError>> {
  x.with_literal(predicate)
    .map_err(|_| AssertionError::ext(x.clone(), "a literal value"))
    .and_then(|r| r)
}

/// Like [with_lit] but also unwraps [Literal::Str]
pub fn with_str<T>(
  x: &ExprInst,
  predicate: impl FnOnce(&String) -> Result<T, Rc<dyn ExternError>>,
) -> Result<T, Rc<dyn ExternError>> {
  with_lit(x, |l| {
    if let Literal::Str(s) = l {
      predicate(s)
    } else {
      AssertionError::fail(x.clone(), "a string")?
    }
  })
}

/// Like [with_lit] but also unwraps [Literal::Uint]
pub fn with_uint<T>(
  x: &ExprInst,
  predicate: impl FnOnce(u64) -> Result<T, Rc<dyn ExternError>>,
) -> Result<T, Rc<dyn ExternError>> {
  with_lit(x, |l| {
    if let Literal::Uint(u) = l {
      predicate(*u)
    } else {
      AssertionError::fail(x.clone(), "an uint")?
    }
  })
}
