#[allow(unused)] // for the doc comments
use crate::foreign::Atomic;
#[allow(unused)] // for the doc comments
use std::any::Any;
#[allow(unused)] // for the doc comments
use dyn_clone::DynClone;
#[allow(unused)] // for the doc comments
use std::fmt::Debug;

/// Implement [Atomic] for a structure that cannot be transformed any further. This would be optimal
/// for atomics encapsulating raw data. [Atomic] depends on [Any], [Debug] and [DynClone].
#[macro_export]
macro_rules! atomic_inert {
  ($typ:ident) => {
    impl $crate::foreign::Atomic for $typ {
      $crate::atomic_defaults!{}

      fn run(&self, ctx: $crate::interpreter::Context)
      -> $crate::foreign::AtomicResult
      {
        Ok($crate::foreign::AtomicReturn{
          clause: self.clone().to_atom_cls(),
          gas: ctx.gas,
          inert: true
        })
      }
    }
  };
}