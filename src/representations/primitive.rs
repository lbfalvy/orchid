use std::fmt::Debug;

use super::Literal;
use crate::foreign::{Atom, ExternFn};

pub enum Primitive {
  /// A literal value, eg. `1`, `"hello"`
  Literal(Literal),
  /// An opaque function, eg. an effectful function employing CPS.
  ExternFn(Box<dyn ExternFn>),
  /// An opaque non-callable value, eg. a file handle.
  Atom(Atom),
}

impl PartialEq for Primitive {
  fn eq(&self, other: &Self) -> bool {
    if let (Self::Literal(l1), Self::Literal(l2)) = (self, other) {
      l1 == l2
    } else {
      false
    }
  }
}

impl Clone for Primitive {
  fn clone(&self) -> Self {
    match self {
      Primitive::Literal(l) => Primitive::Literal(l.clone()),
      Primitive::Atom(a) => Primitive::Atom(a.clone()),
      Primitive::ExternFn(ef) =>
        Primitive::ExternFn(dyn_clone::clone_box(ef.as_ref())),
    }
  }
}

impl Debug for Primitive {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Atom(a) => write!(f, "{a:?}"),
      Self::ExternFn(ef) => write!(f, "{ef:?}"),
      Self::Literal(l) => write!(f, "{l:?}"),
    }
  }
}
