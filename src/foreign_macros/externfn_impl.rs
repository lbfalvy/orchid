#[allow(unused)] // for the doc comments
use crate::{atomic_impl, atomic_redirect};
#[allow(unused)] // for the doc comments
use crate::representations::Primitive;
#[allow(unused)] // for the doc comments
use crate::foreign::{Atomic, ExternFn};
#[allow(unused)] // for the doc comments
use std::any::Any;
#[allow(unused)] // for the doc comments
use std::hash::Hash;
#[allow(unused)] // for the doc comments
use dyn_clone::DynClone;
#[allow(unused)] // for the doc comments
use std::fmt::Debug;

/// Implement [ExternFn] with a closure that produces an [Atomic] from a reference to self
/// and a closure. This can be used in conjunction with [atomic_impl] and [atomic_redirect]
/// to normalize the argument automatically before using it.
#[macro_export]
macro_rules! externfn_impl {
  ($typ:ident, $next_atomic:expr) => {
    impl $crate::foreign::ExternFn for $typ {
      fn name(&self) -> &str {stringify!($typ)}
      fn apply(&self,
        arg: $crate::foreign::RcExpr,
        _ctx: $crate::interpreter::Context
      ) -> $crate::foreign::XfnResult {
        match ($next_atomic)(self, arg) { // ? casts the result but we want to strictly forward it
          Ok(r) => Ok(
            $crate::representations::interpreted::Clause::P(
              $crate::representations::Primitive::Atom(
                $crate::foreign::Atom::new(r)
              )
            )
          ),
          Err(e) => Err(e)
        }
      }
    }
  };
}