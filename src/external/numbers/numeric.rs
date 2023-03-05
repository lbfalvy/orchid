use std::ops::{Add, Sub, Mul, Div, Rem};
use std::rc::Rc;

use ordered_float::NotNan;

use crate::external::assertion_error::AssertionError;
use crate::foreign::ExternError;
use crate::representations::{Primitive, Literal};
use crate::representations::interpreted::Clause;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Numeric {
  Int(u64),
  Num(NotNan<f64>)
}

impl Add for Numeric {
  type Output = Numeric;

  fn add(self, rhs: Self) -> Self::Output {
    match (self, rhs) {
      (Numeric::Int(a), Numeric::Int(b)) => Numeric::Int(a + b),
      (Numeric::Num(a), Numeric::Num(b)) => Numeric::Num(a + b),
      (Numeric::Int(a), Numeric::Num(b)) | (Numeric::Num(b), Numeric::Int(a))
        => Numeric::Num(NotNan::new(a as f64).unwrap() + b)
    }
  }
}

impl Sub for Numeric {
  type Output = Numeric;

  fn sub(self, rhs: Self) -> Self::Output {
    match (self, rhs) {
      (Numeric::Int(a), Numeric::Int(b)) if b < a => Numeric::Int(a - b),
      (Numeric::Int(a), Numeric::Int(b))
        => Numeric::Num(NotNan::new(a as f64 - b as f64).unwrap()),
      (Numeric::Num(a), Numeric::Num(b)) => Numeric::Num(a - b),
      (Numeric::Int(a), Numeric::Num(b)) | (Numeric::Num(b), Numeric::Int(a))
        => Numeric::Num(NotNan::new(a as f64).unwrap() - b)
    }
  }
}

impl Mul for Numeric {
  type Output = Numeric;

  fn mul(self, rhs: Self) -> Self::Output {
    match (self, rhs) {
      (Numeric::Int(a), Numeric::Int(b)) => Numeric::Int(a * b),
      (Numeric::Num(a), Numeric::Num(b)) => Numeric::Num(a * b),
      (Numeric::Int(a), Numeric::Num(b)) | (Numeric::Num(b), Numeric::Int(a))
        => Numeric::Num(NotNan::new(a as f64).unwrap() * b)
    }
  }
}

impl Div for Numeric {
  type Output = Numeric;

  fn div(self, rhs: Self) -> Self::Output {
    let a = match self { Numeric::Int(i) => i as f64, Numeric::Num(f) => *f };
    let b = match rhs { Numeric::Int(i) => i as f64, Numeric::Num(f) => *f };
    Numeric::Num(NotNan::new(a / b).unwrap())
  }
}

impl Rem for Numeric {
  type Output = Numeric;

  fn rem(self, rhs: Self) -> Self::Output {
    match (self, rhs) {
      (Numeric::Int(a), Numeric::Int(b)) => Numeric::Int(a % b),
      (Numeric::Num(a), Numeric::Num(b)) => Numeric::Num(a % b),
      (Numeric::Int(a), Numeric::Num(b)) | (Numeric::Num(b), Numeric::Int(a))
        => Numeric::Num(NotNan::new(a as f64).unwrap() % b)
    }
  }
}

impl TryFrom<Clause> for Numeric {
  type Error = Rc<dyn ExternError>;
  fn try_from(value: Clause) -> Result<Self, Self::Error> {
    let l = if let Clause::P(Primitive::Literal(l)) = value.clone() {l} else {
      AssertionError::fail(value, "a literal value")?
    };
    match l {
      Literal::Int(i) => Ok(Numeric::Int(i)),
      Literal::Num(n) => Ok(Numeric::Num(n)),
      _ => AssertionError::fail(value, "an integer or number")?
    }
  }
}

impl From<Numeric> for Clause {
  fn from(value: Numeric) -> Self {
    Clause::P(Primitive::Literal(match value {
      Numeric::Int(i) => Literal::Int(i),
      Numeric::Num(n) => Literal::Num(n)
    }))
  }
}