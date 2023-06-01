#[allow(unused)] // for doc
use crate::foreign::ExternFn;
#[allow(unused)] // for doc
use crate::interpreted::ExprInst;

/// Write one step in the state machine representing a simple n-ary non-variadic
/// Orchid function.
///
/// There are three ways to call this macro for the initial state, internal
/// state, and exit state. All of them are demonstrated in one example and
/// discussed below.
///
/// ```
/// use orchidlang::{write_fn_step, Literal, Primitive};
/// use orchidlang::interpreted::Clause;
/// use orchidlang::stl::litconv::{with_str, with_uint};
/// use orchidlang::stl::RuntimeError;
///
/// // Initial state
/// write_fn_step!(pub CharAt2 > CharAt1);
/// // Middle state
/// write_fn_step!(
///   CharAt1 {}
///   CharAt0 where s = |x| with_str(x, |s| Ok(s.clone()))
/// );
/// // Exit state
/// write_fn_step!(
///   CharAt0 { s: String }
///   i = |x| with_uint(x, Ok)
///   => {
///     if let Some(c) = s.chars().nth(i as usize) {
///       Ok(Clause::P(Primitive::Literal(Literal::Char(c))))
///     } else {
///       RuntimeError::fail(
///         "Character index out of bounds".to_string(),
///         "indexing string",
///       )?
///     }
///   }
/// );
/// ```
///
/// The initial state simply defines an empty marker struct and implements
/// [ExternFn] on it, transitioning into a new struct which is assumed to have a
/// single field called `expr_inst` of type [ExprInst].
///
/// The middle state defines a sequence of arguments with types similarly to a
/// struct definition. A field called `expr_inst` of type [ExprInst] is added
/// implicitly, so the first middle state has an empty field list. The next
/// state is also provided, alongside the name and conversion function of the
/// next parameter which is [FnOnce(&ExprInst) -> Result<_, RuntimeError>]. The
/// success type is inferred from the type of the field at the place of its
/// actual definition. This conversion is done in the implementation of
/// [ExternFn] which also places the new [ExprInst] into `expr_inst` on the next
/// state.
///
/// The final state defines the sequence of all arguments except for the last
/// one with the same syntax used by the middle state, and the name and
/// conversion lambda of the final argument without specifying the type - it is
/// to be inferred. This state also specifies the operation that gets executed
/// when all the arguments are collected. Uniquely, this "function body" isn't
/// specified as a lambda but rather as an expression invoked with all the
/// argument names bound. The arguments here are all references to their actual
/// types except for the last one which is converted from [ExprInst] immediately
/// before the body is evaluated.
#[macro_export]
macro_rules! write_fn_step {
  ($quant:vis $name:ident > $next:ident) => {
    #[derive(Clone)]
    $quant struct $name;
    $crate::externfn_impl!{
      $name,
      |_: &Self, expr_inst: $crate::interpreted::ExprInst| {
        Ok($next{ expr_inst })
      }
    }
  };
  (
    $quant:vis $name:ident {
      $( $arg:ident : $typ:ty ),*
    }
    $next:ident where $added:ident = $extract:expr
  ) => {
    #[derive(std::fmt::Debug, Clone)]
    $quant struct $name {
      $( $arg: $typ, )*
      expr_inst: $crate::interpreted::ExprInst,
    }
    $crate::atomic_redirect!($name, expr_inst);
    $crate::atomic_impl!($name);
    $crate::externfn_impl!(
      $name,
      |this: &Self, expr_inst: $crate::interpreted::ExprInst| {
        let lambda = $extract;
        Ok($next{
          $( $arg: this.$arg.clone(), )*
          $added: lambda(&this.expr_inst)?,
          expr_inst
        })
      }
    );
  };
  (
    $quant:vis $name:ident {
      $( $arg:ident: $typ:ty ),*
    }
    $added:ident = $extract:expr
    => $process:expr
  ) => {
    #[derive(std::fmt::Debug, Clone)]
    $quant struct $name {
      $( $arg: $typ, )+
      expr_inst: $crate::interpreted::ExprInst,
    }
    $crate::atomic_redirect!($name, expr_inst);
    $crate::atomic_impl!(
      $name,
      |Self{ $($arg, )* expr_inst }: &Self, _| {
        let lambda = $extract;
        let $added = lambda(expr_inst)?;
        $process
      }
    );
  };
}