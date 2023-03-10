#[allow(unused)] // for the doc comments
use crate::representations::Primitive;
#[allow(unused)] // for the doc comments
use crate::foreign::{Atomic, ExternFn};
#[allow(unused)] // for the doc comments
use std::any::Any;
#[allow(unused)] // for the doc comments
use dyn_clone::DynClone;
#[allow(unused)] // for the doc comments
use std::fmt::Debug;

/// A macro that generates implementations of [Atomic] to simplify the development of
/// external bindings for Orchid.
/// 
/// The macro depends on implementations of [AsRef<Clause>] and [From<(&Self, Clause)>] for
/// extracting the clause to be processed and then reconstructing the [Atomic]. Naturally,
/// supertraits of [Atomic] are also dependencies. These are [Any], [Debug] and [DynClone].
/// 
/// The simplest form just requires the typename to be specified. This additionally depends on an
/// implementation of [ExternFn] because after the clause is fully normalized it returns `Self`
/// wrapped in a [Primitive::ExternFn]. It is intended for intermediary
/// stages of the function where validation and the next state are defined in [ExternFn::apply].
/// 
/// ```
/// atomic_impl!(Multiply1)
/// ```
/// 
/// The last stage of the function should use the extended form of the macro which takes an
/// additional closure to explicitly describe what happens when the argument is fully processed.
/// 
/// ```
/// // excerpt from the exact implementation of Multiply
/// atomic_impl!(Multiply0, |Self(a, cls): &Self| {
///   let b: Numeric = cls.clone().try_into().map_err(AssertionError::into_extern)?;
///   Ok(*a * b).into())
/// })
/// ```
/// 
#[macro_export]
macro_rules! atomic_impl {
  ($typ:ident) => {
    atomic_impl!{$typ, |this: &Self| Ok(Clause::P(
      $crate::representations::Primitive::ExternFn(Box::new(this.clone()))
    ))}
  };
  ($typ:ident, $next_phase:expr) => {
    impl $crate::foreign::Atomic for $typ {
      $crate::atomic_defaults!{}
      fn run_once(&self) -> Result<
        $crate::representations::interpreted::Clause,
        $crate::representations::interpreted::InternalError
      > {
        match <Self as AsRef<$crate::representations::interpreted::Clause>>::as_ref(self).run_once() {
          Err($crate::representations::interpreted::InternalError::NonReducible) => {
            ($next_phase)(self)
              .map_err($crate::representations::interpreted::RuntimeError::Extern)
              .map_err($crate::representations::interpreted::InternalError::Runtime)
          }
          Ok(arg) => Ok($crate::representations::interpreted::Clause::P(
            $crate::representations::Primitive::Atom(
              $crate::foreign::Atom::new(
                <Self as From<(&Self, Clause)>>::from((self, arg))
              )
            )
          )),
          Err(e) => Err(e),
        }
      }
      fn run_n_times(&self, n: usize) -> Result<
        (
          $crate::representations::interpreted::Clause,
          usize
        ),
        $crate::representations::interpreted::RuntimeError
      > {
        match <Self as AsRef<Clause>>::as_ref(self).run_n_times(n) {
          Ok((arg, k)) if k == n => Ok((Clause::P(
            $crate::representations::Primitive::Atom(
              $crate::foreign::Atom::new(
                <Self as From<(&Self, Clause)>>::from((self, arg))
              )
            )
          ), k)),
          Ok((arg, k)) => {
            let intermediate = <Self as From<(&Self, Clause)>>::from((self, arg));
            ($next_phase)(&intermediate)
              .map(|cls| (cls, k))
              .map_err($crate::representations::interpreted::RuntimeError::Extern)
          }
          Err(e) => Err(e),
        }
      }
      fn run_to_completion(&self) -> Result<Clause, $crate::representations::interpreted::RuntimeError> {
        match <Self as AsRef<Clause>>::as_ref(self).run_to_completion() {
          Ok(arg) => {
            let intermediate = <Self as From<(&Self, Clause)>>::from((self, arg));
            ($next_phase)(&intermediate)
              .map_err($crate::representations::interpreted::RuntimeError::Extern)
          },
          Err(e) => Err(e)
        }
      }
    }
  };
}