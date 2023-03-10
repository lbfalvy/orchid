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
      fn run_once(&self) -> Result<
        $crate::representations::interpreted::Clause,
        $crate::representations::interpreted::InternalError
      > {
        Err($crate::representations::interpreted::InternalError::NonReducible)
      }
      fn run_n_times(&self, _: usize) -> Result<
        (
          $crate::representations::interpreted::Clause,
          usize
        ),
        $crate::representations::interpreted::RuntimeError
      > {
        Ok(($crate::representations::interpreted::Clause::P(
          $crate::representations::Primitive::Atom(
            $crate::foreign::Atom::new(self.clone())
          )
        ), 0))
      }
      fn run_to_completion(&self) -> Result<
        $crate::representations::interpreted::Clause,
        $crate::representations::interpreted::RuntimeError
      > {
        Ok($crate::representations::interpreted::Clause::P(
          $crate::representations::Primitive::Atom(
            $crate::foreign::Atom::new(self.clone())
          )
        ))
      }
    }
  };
}