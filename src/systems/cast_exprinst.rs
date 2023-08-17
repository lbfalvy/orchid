//! Utility functions that operate on literals. Because of the parallel locked
//! nature of [ExprInst], returning a reference to [Literal] is not possible.
use std::rc::Rc;

use ordered_float::NotNan;

use super::assertion_error::AssertionError;
use crate::foreign::{Atomic, ExternError};
use crate::interpreted::Clause;
use crate::representations::interpreted::ExprInst;
use crate::representations::Literal;
use crate::Primitive;

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

/// Like [with_lit] but also unwraps [Literal::Num]
pub fn with_num<T>(
  x: &ExprInst,
  predicate: impl FnOnce(NotNan<f64>) -> Result<T, Rc<dyn ExternError>>,
) -> Result<T, Rc<dyn ExternError>> {
  with_lit(x, |l| {
    if let Literal::Num(n) = l {
      predicate(*n)
    } else {
      AssertionError::fail(x.clone(), "a float")?
    }
  })
}

/// Tries to cast the [ExprInst] into the specified atom type. Throws an
/// assertion error if unsuccessful, or calls the provided function on the
/// extracted atomic type.
pub fn with_atom<T: Atomic, U>(
  x: &ExprInst,
  inexact_typename: &'static str,
  predicate: impl FnOnce(&T) -> Result<U, Rc<dyn ExternError>>,
) -> Result<U, Rc<dyn ExternError>> {
  x.inspect(|c| {
    if let Clause::P(Primitive::Atom(a)) = c {
      a.try_cast()
        .map(predicate)
        .unwrap_or_else(|| AssertionError::fail(x.clone(), inexact_typename))
    } else {
      AssertionError::fail(x.clone(), "an atom")
    }
  })
}

// ######## Automatically ########

impl TryFrom<&ExprInst> for Literal {
  type Error = Rc<dyn ExternError>;

  fn try_from(value: &ExprInst) -> Result<Self, Self::Error> {
    with_lit(value, |l| Ok(l.clone()))
  }
}

impl TryFrom<&ExprInst> for String {
  type Error = Rc<dyn ExternError>;

  fn try_from(value: &ExprInst) -> Result<Self, Self::Error> {
    with_str(value, |s| Ok(s.clone()))
  }
}

impl TryFrom<&ExprInst> for u64 {
  type Error = Rc<dyn ExternError>;

  fn try_from(value: &ExprInst) -> Result<Self, Self::Error> {
    with_uint(value, Ok)
  }
}

impl TryFrom<&ExprInst> for NotNan<f64> {
  type Error = Rc<dyn ExternError>;

  fn try_from(value: &ExprInst) -> Result<Self, Self::Error> {
    with_num(value, Ok)
  }
}
