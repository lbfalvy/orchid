use std::rc::Rc;

use ordered_float::NotNan;

use super::ArithmeticError;
use crate::error::AssertionError;
use crate::foreign::{xfn_2ary, Atomic, ExternError, ToClause, XfnResult};
use crate::interpreted::TryFromExprInst;
use crate::representations::interpreted::{Clause, ExprInst};
use crate::{ConstTree, Interner, Location};

// region:  Numeric, type to handle floats and uints together

/// A number, either floating point or unsigned int, visible to Orchid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Numeric {
  /// A nonnegative integer such as a size, index or count
  Uint(usize),
  /// A float other than NaN. Orchid has no silent errors
  Float(NotNan<f64>),
}

impl Numeric {
  /// Return the enclosed float, or cast the enclosed int to a float
  pub fn as_f64(&self) -> f64 {
    match self {
      Numeric::Float(n) => **n,
      Numeric::Uint(i) => *i as f64,
    }
  }

  /// Returns the enclosed [NotNan], or casts and wraps the enclosed int
  pub fn as_float(&self) -> NotNan<f64> {
    match self {
      Numeric::Float(n) => *n,
      Numeric::Uint(i) =>
        NotNan::new(*i as f64).expect("ints cannot cast to NaN"),
    }
  }

  /// Wrap a f64 in a Numeric
  pub fn new(value: f64) -> Result<Self, Rc<dyn ExternError>> {
    if value.is_finite() {
      NotNan::new(value)
        .map(Self::Float)
        .map_err(|_| ArithmeticError::NaN.into_extern())
    } else {
      Err(ArithmeticError::Infinity.into_extern())
    }
  }
}
impl TryFromExprInst for Numeric {
  fn from_exi(exi: ExprInst) -> Result<Self, Rc<dyn ExternError>> {
    (exi.request())
      .ok_or_else(|| AssertionError::ext(Location::Unknown, "a numeric value"))
  }
}

impl ToClause for Numeric {
  fn to_clause(self) -> Clause {
    match self {
      Numeric::Uint(i) => i.atom_cls(),
      Numeric::Float(n) => n.atom_cls(),
    }
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
    (Numeric::Float(a), Numeric::Float(b)) => Numeric::new(*(a + b)),
    (Numeric::Float(a), Numeric::Uint(b))
    | (Numeric::Uint(b), Numeric::Float(a)) => Numeric::new(*a + b as f64),
  }
}

/// Subtract a number from another. Always returns Number.
pub fn subtract(a: Numeric, b: Numeric) -> XfnResult<Numeric> {
  match (a, b) {
    (Numeric::Uint(a), Numeric::Uint(b)) => Numeric::new(a as f64 - b as f64),
    (Numeric::Float(a), Numeric::Float(b)) => Numeric::new(*(a - b)),
    (Numeric::Float(a), Numeric::Uint(b)) => Numeric::new(*a - b as f64),
    (Numeric::Uint(a), Numeric::Float(b)) => Numeric::new(a as f64 - *b),
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
    (Numeric::Float(a), Numeric::Float(b)) => Numeric::new(*(a * b)),
    (Numeric::Uint(a), Numeric::Float(b))
    | (Numeric::Float(b), Numeric::Uint(a)) => Numeric::new(a as f64 * *b),
  }
}

/// Divide a number by another. Always returns Number.
pub fn divide(a: Numeric, b: Numeric) -> XfnResult<Numeric> {
  let a: f64 = a.as_f64();
  let b: f64 = b.as_f64();
  if b == 0.0 {
    return Err(ArithmeticError::DivByZero.into_extern());
  }
  Numeric::new(a / b)
}

/// Take the remainder of two numbers.  If they're both uint, the output is
/// uint. If either is number, the output is number.
pub fn remainder(a: Numeric, b: Numeric) -> XfnResult<Numeric> {
  match (a, b) {
    (Numeric::Uint(a), Numeric::Uint(b)) => a
      .checked_rem(b)
      .map(Numeric::Uint)
      .ok_or_else(|| ArithmeticError::DivByZero.into_extern()),
    (Numeric::Float(a), Numeric::Float(b)) => Numeric::new(*(a % b)),
    (Numeric::Uint(a), Numeric::Float(b)) => Numeric::new(a as f64 % *b),
    (Numeric::Float(a), Numeric::Uint(b)) => Numeric::new(*a % b as f64),
  }
}

pub fn less_than(a: Numeric, b: Numeric) -> XfnResult<bool> {
  Ok(match (a, b) {
    (Numeric::Uint(a), Numeric::Uint(b)) => a < b,
    (a, b) => a.as_f64() < b.as_f64(),
  })
}

// endregion

pub fn num(i: &Interner) -> ConstTree {
  ConstTree::tree([(
    i.i("number"),
    ConstTree::tree([
      (i.i("add"), ConstTree::xfn(xfn_2ary(add))),
      (i.i("subtract"), ConstTree::xfn(xfn_2ary(subtract))),
      (i.i("multiply"), ConstTree::xfn(xfn_2ary(multiply))),
      (i.i("divide"), ConstTree::xfn(xfn_2ary(divide))),
      (i.i("remainder"), ConstTree::xfn(xfn_2ary(remainder))),
      (i.i("less_than"), ConstTree::xfn(xfn_2ary(less_than))),
    ]),
  )])
}
