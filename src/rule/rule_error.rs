use std::fmt;

use crate::interner::{Token, InternedDisplay, Interner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleError {
  Missing(Token<String>),
  TypeMismatch(Token<String>),
  /// Multiple occurences of a placeholder in a pattern are no longer
  /// supported.
  Multiple(Token<String>),
  VecNeighbors(Token<String>, Token<String>),
}

impl InternedDisplay for RuleError {
  fn fmt_i(&self, f: &mut fmt::Formatter<'_>, i: &Interner) -> fmt::Result {
    match *self {
      Self::Missing(key) => write!(f,
        "Key {:?} not in match pattern",
        i.r(key)
      ),
      Self::TypeMismatch(key) => write!(f,
        "Key {:?} used inconsistently with and without ellipsis",
        i.r(key)
      ),
      Self::Multiple(key) => write!(f,
        "Key {:?} appears multiple times in match pattern",
        i.r(key)
      ),
      Self::VecNeighbors(left, right) => write!(f,
        "Keys {:?} and {:?} are two vectorials right next to each other",
        i.r(left), i.r(right)
      )
    }
  }
}