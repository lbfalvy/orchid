
use crate::{atomic_inert, representations::{interpreted::Clause, Primitive}, foreign::Atom};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Boolean(pub bool);
atomic_inert!(Boolean);

impl From<bool> for Boolean { fn from(value: bool) -> Self { Self(value) } }

impl<'a> TryFrom<&'a Clause> for Boolean {
  type Error = ();

  fn try_from(value: &'a Clause) -> Result<Self, Self::Error> {
    if let Clause::P(Primitive::Atom(Atom(a))) = value {
      if let Some(b) = a.as_any().downcast_ref::<Boolean>() {
        return Ok(*b)
      }
    }
    return Err(())
  }
}