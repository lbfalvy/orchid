use std::rc::Rc;

use ordered_float::NotNan;

use super::ArithmeticError;
use crate::foreign::{xfn_2ary, ExternError, ToClause, XfnResult};
use crate::interpreted::TryFromExprInst;
use crate::representations::interpreted::{Clause, ExprInst};
use crate::representations::{Literal, Primitive};
use crate::systems::cast_exprinst::get_literal;
use crate::systems::AssertionError;
use crate::{ConstTree, Interner};

// region:  Numeric, type to handle floats and uints together

/// A number, either floating point or unsigned int, visible to Orchid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Numeric {
  /// A nonnegative integer such as a size, index or count
  Uint(u64),
  /// A float other than NaN. Orchid has no silent errors
  Num(NotNan<f64>),
}

impl Numeric {
  fn as_f64(&self) -> f64 {
    match self {
      Numeric::Num(n) => **n,
      Numeric::Uint(i) => *i as f64,
    }
  }

  /// Wrap a f64 in a Numeric
  fn num(value: f64) -> Result<Self, Rc<dyn ExternError>> {
    if value.is_finite() {
      NotNan::new(value)
        .map(Self::Num)
        .map_err(|_| ArithmeticError::NaN.into_extern())
    } else {
      Err(ArithmeticError::Infinity.into_extern())
    }
  }
}
impl TryFromExprInst for Numeric {
  fn from_exi(exi: ExprInst) -> Result<Self, Rc<dyn ExternError>> {
    match get_literal(exi)? {
      (Literal::Uint(i), _) => Ok(Numeric::Uint(i)),
      (Literal::Num(n), _) => Ok(Numeric::Num(n)),
      (_, location) => AssertionError::fail(location, "an integer or number")?,
    }
  }
}

impl ToClause for Numeric {
  fn to_clause(self) -> Clause {
    Clause::P(Primitive::Literal(match self {
      Numeric::Uint(i) => Literal::Uint(i),
      Numeric::Num(n) => Literal::Num(n),
    }))
  }
}

// endregion

/// Add two numbers. If they're both uint, the output is uint. If either is
/// number, the output is number.
pub fn add(a: Numeric, b: Numeric) -> XfnResult<Numeric> {
  match (a, b) {
    (Numeric::Uint(a), Numeric::Uint(b)) => a
      .checked_add(b)
      .map(Numeric::Uint)
      .ok_or_else(|| ArithmeticError::Overflow.into_extern()),
    (Numeric::Num(a), Numeric::Num(b)) => Numeric::num(*(a + b)),
    (Numeric::Num(a), Numeric::Uint(b))
    | (Numeric::Uint(b), Numeric::Num(a)) => Numeric::num(*a + b as f64),
  }
}

/// Subtract a number from another. Always returns Number.
pub fn subtract(a: Numeric, b: Numeric) -> XfnResult<Numeric> {
  match (a, b) {
    (Numeric::Uint(a), Numeric::Uint(b)) => Numeric::num(a as f64 - b as f64),
    (Numeric::Num(a), Numeric::Num(b)) => Numeric::num(*(a - b)),
    (Numeric::Num(a), Numeric::Uint(b)) => Numeric::num(*a - b as f64),
    (Numeric::Uint(a), Numeric::Num(b)) => Numeric::num(a as f64 - *b),
  }
}

/// Multiply two numbers. If they're both uint, the output is uint. If either
/// is number, the output is number.
pub fn multiply(a: Numeric, b: Numeric) -> XfnResult<Numeric> {
  match (a, b) {
    (Numeric::Uint(a), Numeric::Uint(b)) => a
      .checked_mul(b)
      .map(Numeric::Uint)
      .ok_or_else(|| ArithmeticError::Overflow.into_extern()),
    (Numeric::Num(a), Numeric::Num(b)) => Numeric::num(*(a * b)),
    (Numeric::Uint(a), Numeric::Num(b))
    | (Numeric::Num(b), Numeric::Uint(a)) => Numeric::num(a as f64 * *b),
  }
}

/// Divide a number by another. Always returns Number.
pub fn divide(a: Numeric, b: Numeric) -> XfnResult<Numeric> {
  let a: f64 = a.as_f64();
  let b: f64 = b.as_f64();
  if b == 0.0 {
    return Err(ArithmeticError::DivByZero.into_extern());
  }
  Numeric::num(a / b)
}

/// Take the remainder of two numbers.  If they're both uint, the output is
/// uint. If either is number, the output is number.
pub fn remainder(a: Numeric, b: Numeric) -> XfnResult<Numeric> {
  match (a, b) {
    (Numeric::Uint(a), Numeric::Uint(b)) => a
      .checked_rem(b)
      .map(Numeric::Uint)
      .ok_or_else(|| ArithmeticError::DivByZero.into_extern()),
    (Numeric::Num(a), Numeric::Num(b)) => Numeric::num(*(a % b)),
    (Numeric::Uint(a), Numeric::Num(b)) => Numeric::num(a as f64 % *b),
    (Numeric::Num(a), Numeric::Uint(b)) => Numeric::num(*a % b as f64),
  }
}

// endregion

pub fn num(i: &Interner) -> ConstTree {
  ConstTree::tree([(
    i.i("num"),
    ConstTree::tree([
      (i.i("add"), ConstTree::xfn(xfn_2ary(add))),
      (i.i("subtract"), ConstTree::xfn(xfn_2ary(subtract))),
      (i.i("multiply"), ConstTree::xfn(xfn_2ary(multiply))),
      (i.i("divide"), ConstTree::xfn(xfn_2ary(divide))),
      (i.i("remainder"), ConstTree::xfn(xfn_2ary(remainder))),
    ]),
  )])
}
