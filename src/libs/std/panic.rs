use std::fmt::Display;
use std::sync::Arc;

use never::Never;

use super::string::OrcString;
use crate::foreign::error::{ExternError, ExternResult};
use crate::foreign::inert::Inert;
use crate::gen::tree::{xfn_leaf, ConstTree}; 

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
pub fn orc_panic(msg: Inert<OrcString>) -> ExternResult<Never> {
  // any return value would work, but Clause is the simplest
  Err(OrchidPanic(Arc::new(msg.0.get_string())).rc())
}

pub fn panic_lib() -> ConstTree { ConstTree::ns("std::panic", [xfn_leaf(orc_panic)]) }
