use std::fmt::Display;
use std::sync::Arc;

use crate::foreign::{xfn_1ary, ExternError, XfnResult};
use crate::interpreted::Clause;
use crate::{ConstTree, Interner, OrcString};

/// An unrecoverable error in Orchid land. Because Orchid is lazy, this only
/// invalidates expressions that reference the one that generated it.
#[derive(Clone)]
pub struct OrchidPanic(Arc<String>);

impl Display for OrchidPanic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Orchid code panicked: {}", self.0)
  }
}

impl ExternError for OrchidPanic {}

/// Takes a message, returns an [ExternError] unconditionally.
pub fn orc_panic(msg: OrcString) -> XfnResult<Clause> {
  // any return value would work, but Clause is the simplest
  Err(OrchidPanic(Arc::new(msg.get_string())).into_extern())
}

pub fn panic(i: &Interner) -> ConstTree {
  ConstTree::tree([(i.i("panic"), ConstTree::xfn(xfn_1ary(orc_panic)))])
}
