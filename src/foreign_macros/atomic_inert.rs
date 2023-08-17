#[allow(unused)] // for the doc comments
use std::any::Any;
#[allow(unused)] // for the doc comments
use std::fmt::Debug;

#[allow(unused)] // for the doc comments
use dyn_clone::DynClone;

#[allow(unused)] // for the doc comments
use crate::foreign::Atomic;

/// Implement [Atomic] for a structure that cannot be transformed any further.
/// This would be optimal for atomics encapsulating raw data. [Atomic] depends
/// on [Any], [Debug] and [DynClone].
#[macro_export]
macro_rules! atomic_inert {
  ($typ:ident, $typename:expr) => {
    impl $crate::foreign::Atomic for $typ {
      $crate::atomic_defaults! {}

      fn run(
        &self,
        ctx: $crate::interpreter::Context,
      ) -> $crate::foreign::AtomicResult {
        Ok($crate::foreign::AtomicReturn {
          clause: self.clone().atom_cls(),
          gas: ctx.gas,
          inert: true,
        })
      }
    }

    impl TryFrom<&ExprInst> for $typ {
      type Error = std::rc::Rc<dyn $crate::foreign::ExternError>;

      fn try_from(
        value: &$crate::interpreted::ExprInst,
      ) -> Result<Self, Self::Error> {
        $crate::systems::cast_exprinst::with_atom(
          value,
          $typename,
          |a: &$typ| Ok(a.clone()),
        )
      }
    }
  };
}
