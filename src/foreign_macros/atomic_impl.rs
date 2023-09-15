#[allow(unused)] // for the doc comments
use std::any::Any;
#[allow(unused)] // for the doc comments
use std::fmt::Debug;

#[allow(unused)] // for the doc comments
use dyn_clone::DynClone;

#[allow(unused)] // for the doc comments
use crate::define_fn;
#[allow(unused)] // for the doc comments
use crate::foreign::{Atomic, ExternFn};
#[allow(unused)] // for the doc comments
use crate::write_fn_step;
#[allow(unused)] // for the doc comments
use crate::Primitive;

/// A macro that generates implementations of [Atomic] to simplify the
/// development of external bindings for Orchid.
///
/// Most use cases are fulfilled by [define_fn], pathological cases can combine
/// [write_fn_step] with manual [Atomic] implementations.
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
/// use orchidlang::{Literal};
/// use orchidlang::interpreted::{ExprInst, Clause};
/// use orchidlang::systems::cast_exprinst::with_lit;
/// use orchidlang::{atomic_impl, atomic_redirect, externfn_impl};
///
/// /// Convert a literal to a string using Rust's conversions for floats, chars and
/// /// uints respectively
/// #[derive(Clone)]
/// struct ToString;
///
/// externfn_impl!{
///   ToString, |_: &Self, expr_inst: ExprInst|{
///     Ok(InternalToString {
///       expr_inst
///     })
///   }
/// }
/// #[derive(std::fmt::Debug,Clone)]
/// struct InternalToString {
///   expr_inst: ExprInst,
/// }
/// atomic_redirect!(InternalToString, expr_inst);
/// atomic_impl!(InternalToString, |Self { expr_inst }: &Self, _|{
///   with_lit(expr_inst, |l| Ok(match l {
///     Literal::Uint(i) => Literal::Str(i.to_string().into()),
///     Literal::Num(n) => Literal::Str(n.to_string().into()),
///     s@Literal::Str(_) => s.clone(),
///   })).map(Clause::from)
/// });
/// ```
#[macro_export]
macro_rules! atomic_impl {
  ($typ:ident) => {
    $crate::atomic_impl! {$typ, |this: &Self, _: $crate::interpreter::Context| {
      use $crate::foreign::ExternFn;
      Ok(this.clone().xfn_cls())
    }}
  };
  ($typ:ident, $next_phase:expr) => {
    impl $crate::foreign::Atomic for $typ {
      fn as_any(&self) -> &dyn std::any::Any { self }

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
          let res: Result<
            $crate::interpreted::Clause,
            std::rc::Rc<dyn $crate::foreign::ExternError>,
          > = closure(&next_self, ctx);
          match res {
            Ok(r) => r,
            Err(e) => return Err($crate::interpreter::RuntimeError::Extern(e)),
          }
        } else {
          next_self.atom_cls()
        };
        // package and return
        Ok($crate::foreign::AtomicReturn { clause, gas, inert: false })
      }
    }
  };
}
