use std::rc::Rc;

use ordered_float::NotNan;

use super::litconv::with_lit;
use super::{ArithmeticError, AssertionError};
use crate::define_fn;
use crate::foreign::ExternError;
use crate::interner::Interner;
use crate::pipeline::ConstTree;
use crate::representations::interpreted::{Clause, ExprInst};
use crate::representations::{Literal, Primitive};

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

impl TryFrom<&ExprInst> for Numeric {
  type Error = Rc<dyn ExternError>;
  fn try_from(value: &ExprInst) -> Result<Self, Self::Error> {
    with_lit(value, |l| match l {
      Literal::Uint(i) => Ok(Numeric::Uint(*i)),
      Literal::Num(n) => Ok(Numeric::Num(*n)),
      _ => AssertionError::fail(value.clone(), "an integer or number")?,
    })
  }
}

impl From<Numeric> for Clause {
  fn from(value: Numeric) -> Self {
    Clause::P(Primitive::Literal(match value {
      Numeric::Uint(i) => Literal::Uint(i),
      Numeric::Num(n) => Literal::Num(n),
    }))
  }
}

// endregion

// region: operations

define_fn! {
  /// Add two numbers. If they're both uint, the output is uint. If either is
  /// number, the output is number.
  Add { a: Numeric, b: Numeric } => match (a, b) {
    (Numeric::Uint(a), Numeric::Uint(b)) => {
      a.checked_add(*b)
        .map(Numeric::Uint)
        .ok_or_else(|| ArithmeticError::Overflow.into_extern())
    }
    (Numeric::Num(a), Numeric::Num(b)) => Numeric::num(*(a + b)),
    (Numeric::Num(a), Numeric::Uint(b)) | (Numeric::Uint(b), Numeric::Num(a))
    => Numeric::num(a.into_inner() + *b as f64),
  }.map(Numeric::into)
}

define_fn! {
  /// Subtract a number from another. Always returns Number.
  Subtract { a: Numeric, b: Numeric } => match (a, b) {
    (Numeric::Uint(a), Numeric::Uint(b)) => Numeric::num(*a as f64 - *b as f64),
    (Numeric::Num(a), Numeric::Num(b)) => Numeric::num(*(a - b)),
    (Numeric::Num(a), Numeric::Uint(b)) => Numeric::num(**a - *b as f64),
    (Numeric::Uint(a), Numeric::Num(b)) => Numeric::num(*a as f64 - **b),
  }.map(Numeric::into)
}

define_fn! {
  /// Multiply two numbers. If they're both uint, the output is uint. If either
  /// is number, the output is number.
  Multiply { a: Numeric, b: Numeric } => match (a, b) {
    (Numeric::Uint(a), Numeric::Uint(b)) => {
      a.checked_mul(*b)
        .map(Numeric::Uint)
        .ok_or_else(|| ArithmeticError::Overflow.into_extern())
    }
    (Numeric::Num(a), Numeric::Num(b)) => Numeric::num(*(a * b)),
    (Numeric::Uint(a), Numeric::Num(b)) | (Numeric::Num(b), Numeric::Uint(a))
      => Numeric::num(*a as f64 * **b),
  }.map(Numeric::into)
}

define_fn! {
  /// Divide a number by another. Always returns Number.
  Divide { a: Numeric, b: Numeric } => {
    let a: f64 = a.as_f64();
    let b: f64 = b.as_f64();
    if b == 0.0 {
      return Err(ArithmeticError::DivByZero.into_extern())
    }
    Numeric::num(a / b).map(Numeric::into)
  }
}

define_fn! {
  /// Take the remainder of two numbers.  If they're both uint, the output is
  /// uint. If either is number, the output is number.
  Remainder { a: Numeric, b: Numeric } => match (a, b) {
    (Numeric::Uint(a), Numeric::Uint(b)) => {
      a.checked_rem(*b)
        .map(Numeric::Uint)
        .ok_or_else(|| ArithmeticError::DivByZero.into_extern())
    }
    (Numeric::Num(a), Numeric::Num(b)) => Numeric::num(*(a % b)),
    (Numeric::Uint(a), Numeric::Num(b)) => Numeric::num(*a as f64 % **b),
    (Numeric::Num(a), Numeric::Uint(b)) => Numeric::num(**a % *b as f64),
  }.map(Numeric::into)
}

// endregion

pub fn num(i: &Interner) -> ConstTree {
  ConstTree::tree([(
    i.i("num"),
    ConstTree::tree([
      (i.i("add"), ConstTree::xfn(Add)),
      (i.i("subtract"), ConstTree::xfn(Subtract)),
      (i.i("multiply"), ConstTree::xfn(Multiply)),
      (i.i("divide"), ConstTree::xfn(Divide)),
      (i.i("remainder"), ConstTree::xfn(Remainder)),
    ]),
  )])
}