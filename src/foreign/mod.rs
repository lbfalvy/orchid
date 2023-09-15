//! Interaction with foreign code
//!
//! Structures and traits used in the exposure of external functions and values
//! to Orchid code
mod atom;
pub mod cps_box;
mod extern_fn;
mod inert;

use std::rc::Rc;

pub use atom::{Atom, Atomic, AtomicResult, AtomicReturn};
pub use extern_fn::{ExternError, ExternFn, XfnResult};
pub use inert::InertAtomic;

pub use crate::representations::interpreted::Clause;

/// A type-erased error in external code
pub type RcError = Rc<dyn ExternError>;
