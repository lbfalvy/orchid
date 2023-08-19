use std::fmt::Debug;

use ordered_float::NotNan;

use super::OrcString;

/// Exact values read from the AST which have a shared meaning recognized by all
/// external functions
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Literal {
  /// Any floating point number except `NaN`
  Num(NotNan<f64>),
  /// An unsigned integer; a size, index or pointer
  Uint(u64),
  /// A utf-8 character sequence
  Str(OrcString),
}

impl Debug for Literal {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Num(arg0) => write!(f, "{:?}", arg0),
      Self::Uint(arg0) => write!(f, "{:?}", arg0),
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
impl From<String> for Literal {
  fn from(value: String) -> Self {
    Self::Str(value.into())
  }
}
