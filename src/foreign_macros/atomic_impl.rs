#[allow(unused)] // for the doc comments
use std::any::Any;
#[allow(unused)] // for the doc comments
use std::fmt::Debug;

#[allow(unused)] // for the doc comments
use dyn_clone::DynClone;

#[allow(unused)] // for the doc comments
use crate::foreign::{Atomic, ExternFn};
#[allow(unused)] // for the doc comments
use crate::Primitive;

/// A macro that generates implementations of [Atomic] to simplify the
/// development of external bindings for Orchid.
///
/// The macro depends on implementations of [`AsRef<Clause>`] and
/// [`From<(&Self, Clause)>`] for extracting the clause to be processed and then
/// reconstructing the [Atomic]. Naturally, supertraits of [Atomic] are also
/// dependencies. These are [Any], [Debug] and [Clone].
///
/// The simplest form just requires the typename to be specified. This
/// additionally depends on an implementation of [ExternFn] because after the
/// clause is fully normalized it returns `Self` wrapped in a
/// [Primitive::ExternFn]. It is intended for intermediary stages of the
/// function where validation and the next state are defined in
/// [ExternFn::apply].
///
/// The last stage of the function should use the extended form of the macro
/// which takes an additional closure to explicitly describe what happens when
/// the argument is fully processed.
///
/// _definition of the `add` function in the STL_
/// ```
/// use orchidlang::interpreted::ExprInst;
/// use orchidlang::stl::Numeric;
/// use orchidlang::{atomic_impl, atomic_redirect, externfn_impl};
///
/// #[derive(Clone)]
/// pub struct Add2;
/// externfn_impl!(Add2, |_: &Self, x: ExprInst| Ok(Add1 { x }));
///
/// #[derive(Debug, Clone)]
/// pub struct Add1 {
///   x: ExprInst,
/// }
/// atomic_redirect!(Add1, x);
/// atomic_impl!(Add1);
/// externfn_impl!(Add1, |this: &Self, x: ExprInst| {
///   let a: Numeric = this.x.clone().try_into()?;
///   Ok(Add0 { a, x })
/// });
///
/// #[derive(Debug, Clone)]
/// pub struct Add0 {
///   a: Numeric,
///   x: ExprInst,
/// }
/// atomic_redirect!(Add0, x);
/// atomic_impl!(Add0, |Self { a, x }: &Self, _| {
///   let b: Numeric = x.clone().try_into()?;
///   Ok((*a + b).into())
/// });
/// ```
#[macro_export]
macro_rules! atomic_impl {
  ($typ:ident) => {
    atomic_impl! {$typ, |this: &Self, _: $crate::interpreter::Context| {
      use $crate::foreign::ExternFn;
      Ok(this.clone().to_xfn_cls())
    }}
  };
  ($typ:ident, $next_phase:expr) => {
    impl $crate::foreign::Atomic for $typ {
      $crate::atomic_defaults! {}

      fn run(
        &self,
        ctx: $crate::interpreter::Context,
      ) -> $crate::foreign::AtomicResult {
        // extract the expression
        let expr =
          <Self as AsRef<$crate::interpreted::ExprInst>>::as_ref(self).clone();
        // run the expression
        let ret = $crate::interpreter::run(expr, ctx.clone())?;
        let $crate::interpreter::Return { gas, state, inert } = ret;
        // rebuild the atomic
        let next_self = <Self as From<(
          &Self,
          $crate::interpreted::ExprInst,
        )>>::from((self, state));
        // branch off or wrap up
        let clause = if inert {
          let closure = $next_phase;
          match closure(&next_self, ctx) {
            Ok(r) => r,
            Err(e) => return Err($crate::interpreter::RuntimeError::Extern(e)),
          }
        } else {
          next_self.to_atom_cls()
        };
        // package and return
        Ok($crate::foreign::AtomicReturn { clause, gas, inert: false })
      }
    }
  };
}
