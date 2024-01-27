//! `std::number` Numeric operations.

use ordered_float::NotNan;

use super::arithmetic_error::ArithmeticError;
use crate::foreign::atom::Atomic;
use crate::foreign::error::{AssertionError, ExternError, ExternResult};
use crate::foreign::fn_bridge::constructors::xfn_2ary;
use crate::foreign::inert::Inert;
use crate::foreign::to_clause::ToClause;
use crate::foreign::try_from_expr::TryFromExpr;
use crate::gen::tree::{atom_leaf, ConstTree};
use crate::interpreter::nort::{Clause, Expr};
use crate::location::CodeLocation;

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
  pub fn new(value: f64) -> ExternResult<Self> {
    match value.is_finite() {
      false => Err(ArithmeticError::Infinity.rc()),
      true => match NotNan::new(value) {
        Ok(f) => Ok(Self::Float(f)),
        Err(_) => Err(ArithmeticError::NaN.rc()),
      },
    }
  }
}
impl TryFromExpr for Numeric {
  fn from_expr(exi: Expr) -> ExternResult<Self> {
    (exi.clause.request()).ok_or_else(|| {
      AssertionError::ext(exi.location(), "a numeric value", format!("{exi}"))
    })
  }
}

impl ToClause for Numeric {
  fn to_clause(self, _: CodeLocation) -> Clause {
    match self {
      Numeric::Uint(i) => Inert(i).atom_cls(),
      Numeric::Float(n) => Inert(n).atom_cls(),
    }
  }
}

/// Add two numbers. If they're both uint, the output is uint. If either is
/// number, the output is number.
pub fn add(a: Numeric, b: Numeric) -> ExternResult<Numeric> {
  match (a, b) {
    (Numeric::Uint(a), Numeric::Uint(b)) => a
      .checked_add(b)
      .map(Numeric::Uint)
      .ok_or_else(|| ArithmeticError::Overflow.rc()),
    (Numeric::Float(a), Numeric::Float(b)) => Numeric::new(*(a + b)),
    (Numeric::Float(a), Numeric::Uint(b))
    | (Numeric::Uint(b), Numeric::Float(a)) => Numeric::new(*a + b as f64),
  }
}

/// Subtract a number from another. Always returns Number.
pub fn subtract(a: Numeric, b: Numeric) -> ExternResult<Numeric> {
  match (a, b) {
    (Numeric::Uint(a), Numeric::Uint(b)) => Numeric::new(a as f64 - b as f64),
    (Numeric::Float(a), Numeric::Float(b)) => Numeric::new(*(a - b)),
    (Numeric::Float(a), Numeric::Uint(b)) => Numeric::new(*a - b as f64),
    (Numeric::Uint(a), Numeric::Float(b)) => Numeric::new(a as f64 - *b),
  }
}

/// Multiply two numbers. If they're both uint, the output is uint. If either
/// is number, the output is number.
pub fn multiply(a: Numeric, b: Numeric) -> ExternResult<Numeric> {
  match (a, b) {
    (Numeric::Uint(a), Numeric::Uint(b)) => a
      .checked_mul(b)
      .map(Numeric::Uint)
      .ok_or_else(|| ArithmeticError::Overflow.rc()),
    (Numeric::Float(a), Numeric::Float(b)) => Numeric::new(*(a * b)),
    (Numeric::Uint(a), Numeric::Float(b))
    | (Numeric::Float(b), Numeric::Uint(a)) => Numeric::new(a as f64 * *b),
  }
}

/// Divide a number by another. Always returns Number.
pub fn divide(a: Numeric, b: Numeric) -> ExternResult<Numeric> {
  let a: f64 = a.as_f64();
  let b: f64 = b.as_f64();
  if b == 0.0 {
    return Err(ArithmeticError::DivByZero.rc());
  }
  Numeric::new(a / b)
}

/// Take the remainder of two numbers.  If they're both uint, the output is
/// uint. If either is number, the output is number.
pub fn remainder(a: Numeric, b: Numeric) -> ExternResult<Numeric> {
  match (a, b) {
    (Numeric::Uint(a), Numeric::Uint(b)) => a
      .checked_rem(b)
      .map(Numeric::Uint)
      .ok_or_else(|| ArithmeticError::DivByZero.rc()),
    (Numeric::Float(a), Numeric::Float(b)) => Numeric::new(*(a % b)),
    (Numeric::Uint(a), Numeric::Float(b)) => Numeric::new(a as f64 % *b),
    (Numeric::Float(a), Numeric::Uint(b)) => Numeric::new(*a % b as f64),
  }
}

/// Tries to use integer comparison, casts to float otherwise
pub fn less_than(a: Numeric, b: Numeric) -> Inert<bool> {
  match (a, b) {
    (Numeric::Uint(a), Numeric::Uint(b)) => Inert(a < b),
    (a, b) => Inert(a.as_f64() < b.as_f64()),
  }
}

pub(super) fn num_lib() -> ConstTree {
  ConstTree::ns("std::number", [ConstTree::tree([
    ("add", atom_leaf(xfn_2ary(add))),
    ("subtract", atom_leaf(xfn_2ary(subtract))),
    ("multiply", atom_leaf(xfn_2ary(multiply))),
    ("divide", atom_leaf(xfn_2ary(divide))),
    ("remainder", atom_leaf(xfn_2ary(remainder))),
    ("less_than", atom_leaf(xfn_2ary(less_than))),
  ])])
}
