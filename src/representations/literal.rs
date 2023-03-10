use ordered_float::NotNan;
use std::fmt::Debug;

/// An exact value, read from the AST and unmodified in shape until compilation
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