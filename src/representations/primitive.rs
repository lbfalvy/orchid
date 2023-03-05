use std::fmt::Debug;

use crate::foreign::{ExternFn, Atom};

use super::Literal;

#[derive(Eq, Hash)]
pub enum Primitive {
  /// A literal value, eg. `1`, `"hello"`
  Literal(Literal),
  /// An opaque function, eg. an effectful function employing CPS.
  ExternFn(Box<dyn ExternFn>),
  /// An opaque non-callable value, eg. a file handle.
  Atom(Atom)
}

impl PartialEq for Primitive {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Literal(l1), Self::Literal(l2)) => l1 == l2,
      (Self::Atom(a1), Self::Atom(a2)) => a1 == a2,
      (Self::ExternFn(efb1), Self::ExternFn(efb2)) => efb1 == efb2,
      _ => false
    }
  }
}

impl Clone for Primitive {
  fn clone(&self) -> Self {
    match self {
      Primitive::Literal(l) => Primitive::Literal(l.clone()),
      Primitive::Atom(a) => Primitive::Atom(a.clone()),
      Primitive::ExternFn(ef) => Primitive::ExternFn(
        dyn_clone::clone_box(ef.as_ref())
      )
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

