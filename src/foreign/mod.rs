//! Interaction with foreign code
//!
//! Structures and traits used in the exposure of external functions and values
//! to Orchid code
mod atom;
pub mod cps_box;
mod extern_fn;
mod fn_bridge;
mod inert;

use std::rc::Rc;

pub use atom::{Atom, Atomic, AtomicResult, AtomicReturn, StrictEq};
pub use extern_fn::{ExternError, ExternFn, ExFn};
pub use fn_bridge::constructors::{
  xfn_1ary, xfn_2ary, xfn_3ary, xfn_4ary, xfn_5ary, xfn_6ary, xfn_7ary,
  xfn_8ary, xfn_9ary,
};
pub use fn_bridge::{Param, ToClause};
pub use inert::InertAtomic;

pub use crate::representations::interpreted::Clause;

/// Return type of the argument to the [xfn_1ary] family of functions
pub type XfnResult<T> = Result<T, Rc<dyn ExternError>>;
