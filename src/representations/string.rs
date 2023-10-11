use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;

use crate::foreign::InertAtomic;
use crate::{Interner, Tok};

/// An Orchid string which may or may not be interned
#[derive(Clone, Eq)]
pub enum OrcString {
  /// An interned string. Equality-conpared by reference.
  Interned(Tok<String>),
  /// An uninterned bare string. Equality-compared by character
  Runtime(Arc<String>),
}

impl Debug for OrcString {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Runtime(s) => write!(f, "r\"{s}\""),
      Self::Interned(t) => write!(f, "i\"{t}\""),
    }
  }
}

impl OrcString {
  /// Intern the contained string
  pub fn intern(&mut self, i: &Interner) {
    if let Self::Runtime(t) = self {
      *self = Self::Interned(i.i(t.as_str()))
    }
  }
  /// Clone out the plain Rust [String]
  #[must_use]
  pub fn get_string(self) -> String {
    match self {
      Self::Interned(s) => s.as_str().to_owned(),
      Self::Runtime(rc) =>
        Arc::try_unwrap(rc).unwrap_or_else(|rc| (*rc).clone()),
    }
  }
}

impl Deref for OrcString {
  type Target = String;

  fn deref(&self) -> &Self::Target {
    match self {
      Self::Interned(t) => t,
      Self::Runtime(r) => r,
    }
  }
}

impl Hash for OrcString {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.as_str().hash(state)
  }
}

impl From<String> for OrcString {
  fn from(value: String) -> Self { Self::Runtime(Arc::new(value)) }
}

impl From<Tok<String>> for OrcString {
  fn from(value: Tok<String>) -> Self { Self::Interned(value) }
}

impl PartialEq for OrcString {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Interned(t1), Self::Interned(t2)) => t1 == t2,
      _ => **self == **other,
    }
  }
}

impl InertAtomic for OrcString {
  fn type_str() -> &'static str { "OrcString" }
  fn strict_eq(&self, other: &Self) -> bool { self == other }
}
