//! Utility functions that operate on literals. Because of the parallel locked
//! nature of [ExprInst], returning a reference to [Literal] is not possible.
use std::rc::Rc;

use ordered_float::NotNan;

use super::assertion_error::AssertionError;
use crate::foreign::{Atom, Atomic, ExternError};
use crate::interpreted::{Clause, TryFromExprInst};
use crate::representations::interpreted::ExprInst;
use crate::representations::{Literal, OrcString};
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
  predicate: impl FnOnce(&OrcString) -> Result<T, Rc<dyn ExternError>>,
) -> Result<T, Rc<dyn ExternError>> {
  with_lit(x, |l| match l {
    Literal::Str(s) => predicate(s),
    _ => AssertionError::fail(x.clone(), "a string"),
  })
}

/// If the [ExprInst] stores an [Atom], maps the predicate over it, otherwise
/// raises a runtime error.
pub fn with_atom<T>(
  x: &ExprInst,
  predicate: impl FnOnce(&Atom) -> Result<T, Rc<dyn ExternError>>,
) -> Result<T, Rc<dyn ExternError>> {
  x.inspect(|c| match c {
    Clause::P(Primitive::Atom(a)) => predicate(a),
    _ => AssertionError::fail(x.clone(), "an atom"),
  })
}

/// Tries to cast the [ExprInst] into the specified atom type. Throws an
/// assertion error if unsuccessful, or calls the provided function on the
/// extracted atomic type.
pub fn with_atomic<T: Atomic, U>(
  x: &ExprInst,
  inexact_typename: &'static str,
  predicate: impl FnOnce(&T) -> Result<U, Rc<dyn ExternError>>,
) -> Result<U, Rc<dyn ExternError>> {
  with_atom(x, |a| match a.try_cast() {
    Some(atomic) => predicate(atomic),
    _ => AssertionError::fail(x.clone(), inexact_typename),
  })
}

// ######## Automatically ########

impl TryFromExprInst for Literal {
  fn from_exi(exi: &ExprInst) -> Result<Self, Rc<dyn ExternError>> {
    with_lit(exi, |l| Ok(l.clone()))
  }
}

impl TryFromExprInst for OrcString {
  fn from_exi(exi: &ExprInst) -> Result<Self, Rc<dyn ExternError>> {
    with_str(exi, |s| Ok(s.clone()))
  }
}

impl TryFromExprInst for u64 {
  fn from_exi(exi: &ExprInst) -> Result<Self, Rc<dyn ExternError>> {
    with_lit(exi, |l| match l {
      Literal::Uint(u) => Ok(*u),
      _ => AssertionError::fail(exi.clone(), "an uint"),
    })
  }
}

impl TryFromExprInst for NotNan<f64> {
  fn from_exi(exi: &ExprInst) -> Result<Self, Rc<dyn ExternError>> {
    with_lit(exi, |l| match l {
      Literal::Num(n) => Ok(*n),
      _ => AssertionError::fail(exi.clone(), "a float"),
    })
  }
}
