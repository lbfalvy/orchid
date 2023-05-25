use crate::atomic_inert;
use crate::foreign::Atom;
use crate::representations::interpreted::{Clause, ExprInst};
use crate::representations::Primitive;

/// Booleans exposed to Orchid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Boolean(pub bool);
atomic_inert!(Boolean);

impl From<bool> for Boolean {
  fn from(value: bool) -> Self {
    Self(value)
  }
}

impl TryFrom<ExprInst> for Boolean {
  type Error = ();

  fn try_from(value: ExprInst) -> Result<Self, Self::Error> {
    let expr = value.expr();
    if let Clause::P(Primitive::Atom(Atom(a))) = &expr.clause {
      if let Some(b) = a.as_any().downcast_ref::<Boolean>() {
        return Ok(*b);
      }
    }
    Err(())
  }
}
