use std::fmt::Debug;

use ordered_float::NotNan;

/// An exact value, read from the AST and unmodified in shape until
/// compilation
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Literal {
  Num(NotNan<f64>),
  Uint(u64),
  Char(char),
  Str(String),
}

impl Debug for Literal {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Num(arg0) => write!(f, "{:?}", arg0),
      Self::Uint(arg0) => write!(f, "{:?}", arg0),
      Self::Char(arg0) => write!(f, "{:?}", arg0),
      Self::Str(arg0) => write!(f, "{:?}", arg0),
    }
  }
}

impl From<NotNan<f64>> for Literal {
  fn from(value: NotNan<f64>) -> Self {
    Self::Num(value)
  }
}
impl From<u64> for Literal {
  fn from(value: u64) -> Self {
    Self::Uint(value)
  }
}
impl From<char> for Literal {
  fn from(value: char) -> Self {
    Self::Char(value)
  }
}
impl From<String> for Literal {
  fn from(value: String) -> Self {
    Self::Str(value)
  }
}
