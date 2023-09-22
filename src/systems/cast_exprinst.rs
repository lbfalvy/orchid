//! Utility functions that operate on literals. Because of the parallel locked
//! nature of [ExprInst], returning a reference to [Literal] is not possible.
use std::rc::Rc;

use ordered_float::NotNan;

use super::assertion_error::AssertionError;
use crate::foreign::{Atom, ExternError};
use crate::interpreted::{Clause, Expr, TryFromExprInst};
use crate::representations::interpreted::ExprInst;
use crate::representations::{Literal, OrcString};
use crate::{Location, Primitive};

/// [ExprInst::get_literal] except the error is mapped to an [ExternError]
pub fn get_literal(
  exi: ExprInst,
) -> Result<(Literal, Location), Rc<dyn ExternError>> {
  (exi.get_literal()).map_err(|exi| {
    eprintln!("failed to get literal from {:?}", exi.expr().clause);
    AssertionError::ext(exi.location(), "literal")
  })
}

// ######## Automatically ########

impl TryFromExprInst for Literal {
  fn from_exi(exi: ExprInst) -> Result<Self, Rc<dyn ExternError>> {
    get_literal(exi).map(|(l, _)| l)
  }
}

impl TryFromExprInst for OrcString {
  fn from_exi(exi: ExprInst) -> Result<Self, Rc<dyn ExternError>> {
    match get_literal(exi)? {
      (Literal::Str(s), _) => Ok(s),
      (_, location) => AssertionError::fail(location, "string"),
    }
  }
}

impl TryFromExprInst for u64 {
  fn from_exi(exi: ExprInst) -> Result<Self, Rc<dyn ExternError>> {
    match get_literal(exi)? {
      (Literal::Uint(u), _) => Ok(u),
      (_, location) => AssertionError::fail(location, "uint"),
    }
  }
}

impl TryFromExprInst for NotNan<f64> {
  fn from_exi(exi: ExprInst) -> Result<Self, Rc<dyn ExternError>> {
    match get_literal(exi)? {
      (Literal::Num(n), _) => Ok(n),
      (_, location) => AssertionError::fail(location, "float"),
    }
  }
}

impl TryFromExprInst for Atom {
  fn from_exi(exi: ExprInst) -> Result<Self, Rc<dyn ExternError>> {
    let Expr { clause, location } = exi.expr_val();
    match clause {
      Clause::P(Primitive::Atom(a)) => Ok(a),
      _ => AssertionError::fail(location, "atom"),
    }
  }
}
