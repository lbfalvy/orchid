use std::fmt;
use std::sync::Arc;

use never::Never;

use super::string::OrcString;
use crate::foreign::error::{RTError, RTResult};
use crate::foreign::inert::Inert;
use crate::gen::tree::{xfn_leaf, ConstTree};

/// An unrecoverable error in Orchid land. Because Orchid is lazy, this only
/// invalidates expressions that reference the one that generated it.
#[derive(Clone)]
pub struct OrchidPanic(Arc<String>);

impl fmt::Display for OrchidPanic {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Orchid code panicked: {}", self.0)
  }
}

impl RTError for OrchidPanic {}

/// Takes a message, returns an [ExternError] unconditionally.
pub fn orc_panic(msg: Inert<OrcString>) -> RTResult<Never> {
  // any return value would work, but Clause is the simplest
  Err(OrchidPanic(Arc::new(msg.0.get_string())).pack())
}

pub fn panic_lib() -> ConstTree { ConstTree::ns("std::panic", [xfn_leaf(orc_panic)]) }
