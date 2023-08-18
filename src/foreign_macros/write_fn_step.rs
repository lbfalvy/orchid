#[allow(unused)] // for doc
use crate::define_fn;
#[allow(unused)] // for doc
use crate::foreign::Atomic;
#[allow(unused)] // for doc
use crate::foreign::ExternFn;
#[allow(unused)] // for doc
use crate::interpreted::ExprInst;

/// Write one step in the state machine representing a simple n-ary non-variadic
/// Orchid function. Most use cases are better covered by [define_fn] which
/// generates calls to this macro. This macro can be used in combination with
/// manual [Atomic] implementations to define a function that only behaves like
/// a simple n-ary non-variadic function with respect to some of its arguments.
///
/// There are three ways to call this macro for the initial state, internal
/// state, and exit state. All of them are demonstrated in one example and
/// discussed below. The newly bound names (here `s` and `i` before `=`) can
/// also receive type annotations.
///
/// ```
/// // FIXME this is a very old example that wouldn't compile now
/// use unicode_segmentation::UnicodeSegmentation;
///
/// use orchidlang::{write_fn_step, Literal, Primitive};
/// use orchidlang::interpreted::Clause;
/// use orchidlang::systems::cast_exprinst::{with_str, with_uint};
/// use orchidlang::systems::RuntimeError;
///
/// // Initial state
/// write_fn_step!(pub CharAt2 > CharAt1);
/// // Middle state
/// write_fn_step!(
///   CharAt1 {}
///   CharAt0 where s: String = x => with_str(x, |s| Ok(s.clone()));
/// );
/// // Exit state
/// write_fn_step!(
///   CharAt0 { s: String }
///   i = x => with_uint(x, Ok);
///   {
///     if let Some(c) = s.graphemes(true).nth(*i as usize) {
///       Ok(Literal::Str(c.to_string()).into())
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
/// state is also provided, alongside the name and conversion of the next
/// parameter from a `&ExprInst` under the provided alias to a
/// `Result<_, Rc<dyn ExternError>>`. The success type is inferred from the
/// type of the field at the place of its actual definition. This conversion is
/// done in the implementation of [ExternFn] which also places the new
/// [ExprInst] into `expr_inst` on the next state.
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
///
/// To avoid typing the same parsing process a lot, the conversion is optional.
/// If it is omitted, the field is initialized with a [TryInto::try_into] call
/// from `&ExprInst` to the target type. In this case, the error is
/// short-circuited using `?` so conversions through `FromResidual` are allowed.
/// The optional syntax starts with the `=` sign and ends before the semicolon.
#[macro_export]
macro_rules! write_fn_step {
  // write entry stage
  ( $( #[ $attr:meta ] )* $quant:vis $name:ident > $next:ident) => {
    $( #[ $attr ] )*
    #[derive(Clone)]
    $quant struct $name;
    $crate::externfn_impl!{
      $name,
      |_: &Self, expr_inst: $crate::interpreted::ExprInst| {
        Ok($next{ expr_inst })
      }
    }
  };
  // write middle stage
  (
    $( #[ $attr:meta ] )* $quant:vis $name:ident {
      $( $arg:ident : $typ:ty ),*
    }
    $next:ident where
    $added:ident $( : $added_typ:ty )? $( = $xname:ident => $extract:expr )? ;
  ) => {
    $( #[ $attr ] )*
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
        let $added $( :$added_typ )? =
          $crate::write_fn_step!(@CONV &this.expr_inst $(, $xname $extract )?);
        Ok($next{
          $( $arg: this.$arg.clone(), )*
          $added, expr_inst
        })
      }
    );
  };
  // write final stage
  (
    $( #[ $attr:meta ] )* $quant:vis $name:ident {
      $( $arg:ident: $typ:ty ),*
    }
    $added:ident $(: $added_typ:ty )? $( = $xname:ident => $extract:expr )? ;
    $process:expr
  ) => {
    $( #[ $attr ] )*
    #[derive(std::fmt::Debug, Clone)]
    $quant struct $name {
      $( $arg: $typ, )*
      expr_inst: $crate::interpreted::ExprInst,
    }
    $crate::atomic_redirect!($name, expr_inst);
    $crate::atomic_impl!(
      $name,
      |Self{ $($arg, )* expr_inst }: &Self, _| {
        let added $(: $added_typ )? =
          $crate::write_fn_step!(@CONV expr_inst $(, $xname $extract )?);
        let $added = &added;
        $process
      }
    );
  };
  // Write conversion expression for an ExprInst
  (@CONV $locxname:expr, $xname:ident $extract:expr) => {
    {
      let $xname = $locxname;
      match $extract {
        Err(e) => return Err(e),
        Ok(r) => r,
      }
    }
  };
  (@CONV $locxname:expr) => {
    ($locxname).try_into()?
  };
}
