use std::ops::{Add, Sub, Mul, Div, Rem};
use std::rc::Rc;

use ordered_float::NotNan;

use crate::external::assertion_error::AssertionError;
use crate::external::litconv::with_lit;
use crate::foreign::ExternError;
use crate::representations::Literal;
use crate::representations::Primitive;
use crate::representations::interpreted::{Clause, ExprInst};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Numeric {
  Uint(u64),
  Num(NotNan<f64>)
}

impl Numeric {
  /// Wrap a f64 in a Numeric
  /// 
  /// # Panics
  /// 
  /// if the value is NaN or Infinity.try_into()
  fn num<T>(value: T) -> Self where T: Into<f64> {
    let f = value.into();
    assert!(f.is_finite(), "unrepresentable number");
    NotNan::try_from(f).map(Self::Num).expect("not a number")
  }
}

impl Add for Numeric {
  type Output = Numeric;

  fn add(self, rhs: Self) -> Self::Output {
    match (self, rhs) {
      (Numeric::Uint(a), Numeric::Uint(b)) => Numeric::Uint(a + b),
      (Numeric::Num(a), Numeric::Num(b)) => Numeric::num(a + b),
      (Numeric::Uint(a), Numeric::Num(b)) |
      (Numeric::Num(b), Numeric::Uint(a))
        => Numeric::num::<f64>(a as f64 + *b)
    }
  }
}

impl Sub for Numeric {
  type Output = Numeric;

  fn sub(self, rhs: Self) -> Self::Output {
    match (self, rhs) {
      (Numeric::Uint(a), Numeric::Uint(b)) if b < a => Numeric::Uint(a - b),
      (Numeric::Uint(a), Numeric::Uint(b))
        => Numeric::num(a as f64 - b as f64),
      (Numeric::Num(a), Numeric::Num(b)) => Numeric::num(a - b),
      (Numeric::Uint(a), Numeric::Num(b)) => Numeric::num(a as f64 - *b),
      (Numeric::Num(a), Numeric::Uint(b)) => Numeric::num(*a - b as f64)
    }
  }
}

impl Mul for Numeric {
  type Output = Numeric;

  fn mul(self, rhs: Self) -> Self::Output {
    match (self, rhs) {
      (Numeric::Uint(a), Numeric::Uint(b)) => Numeric::Uint(a * b),
      (Numeric::Num(a), Numeric::Num(b)) => Numeric::num(a * b),
      (Numeric::Uint(a), Numeric::Num(b)) |
      (Numeric::Num(b), Numeric::Uint(a))
        => Numeric::Num(NotNan::new(a as f64).unwrap() * b)
    }
  }
}

impl Div for Numeric {
  type Output = Numeric;

  fn div(self, rhs: Self) -> Self::Output {
    let a: f64 = self.into();
    let b: f64 = rhs.into();
    Numeric::num(a / b)
  }
}

impl Rem for Numeric {
  type Output = Numeric;

  fn rem(self, rhs: Self) -> Self::Output {
    match (self, rhs) {
      (Numeric::Uint(a), Numeric::Uint(b)) => Numeric::Uint(a % b),
      (Numeric::Num(a), Numeric::Num(b)) => Numeric::num(a % b),
      (Numeric::Uint(a), Numeric::Num(b)) => Numeric::num(a as f64 % *b),
      (Numeric::Num(a), Numeric::Uint(b)) => Numeric::num(*a % b as f64)
    }
  }
}

impl TryFrom<ExprInst> for Numeric {
  type Error = Rc<dyn ExternError>;
  fn try_from(value: ExprInst) -> Result<Self, Self::Error> {
    with_lit(&value.clone(), |l| match l {
      Literal::Uint(i) => Ok(Numeric::Uint(*i)),
      Literal::Num(n) => Ok(Numeric::Num(*n)),
      _ => AssertionError::fail(value, "an integer or number")?
    })
  }
}

impl From<Numeric> for Clause {
  fn from(value: Numeric) -> Self {
    Clause::P(Primitive::Literal(match value {
      Numeric::Uint(i) => Literal::Uint(i),
      Numeric::Num(n) => Literal::Num(n)
    }))
  }
}

impl From<Numeric> for String {
  fn from(value: Numeric) -> Self {
    match value {
      Numeric::Uint(i) => i.to_string(),
      Numeric::Num(n) => n.to_string()
    }
  }
}

impl Into<f64> for Numeric {
  fn into(self) -> f64 {
    match self {
      Numeric::Num(n) => *n,
      Numeric::Uint(i) => i as f64
    }
  }
}