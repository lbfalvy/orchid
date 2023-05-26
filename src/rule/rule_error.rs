use std::fmt;

use crate::interner::{InternedDisplay, Interner, Tok};

/// Various reasons why a substitution rule may be invalid
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleError {
  /// A key is present in the template but not the pattern
  Missing(Tok<String>),
  /// A key uses a different arity in the template and in the pattern
  TypeMismatch(Tok<String>),
  /// Multiple occurences of a placeholder in a pattern
  Multiple(Tok<String>),
  /// Two vectorial placeholders are next to each other
  VecNeighbors(Tok<String>, Tok<String>),
}

impl InternedDisplay for RuleError {
  fn fmt_i(&self, f: &mut fmt::Formatter<'_>, i: &Interner) -> fmt::Result {
    match *self {
      Self::Missing(key) =>
        write!(f, "Key {:?} not in match pattern", i.r(key)),
      Self::TypeMismatch(key) => write!(
        f,
        "Key {:?} used inconsistently with and without ellipsis",
        i.r(key)
      ),
      Self::Multiple(key) =>
        write!(f, "Key {:?} appears multiple times in match pattern", i.r(key)),
      Self::VecNeighbors(left, right) => write!(
        f,
        "Keys {:?} and {:?} are two vectorials right next to each other",
        i.r(left),
        i.r(right)
      ),
    }
  }
}
