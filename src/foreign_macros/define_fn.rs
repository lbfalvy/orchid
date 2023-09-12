#[allow(unused)] // for doc
use crate::foreign::ExternFn;
#[allow(unused)] // for doc
use crate::interpreted::ExprInst;
#[allow(unused)] // for doc
use crate::write_fn_step;

/// Define a simple n-ary nonvariadic Orchid function with static argument
/// types.
///
/// This macro relies on [write_fn_step] to define a struct for each step.
/// Because of how Orchid handles state, the arguments must implement [Clone]
/// and [Debug]. All expressions and arguments are accessible as references.
///
/// First, the alias for the newly introduced [ExprInst] is specified. This step
/// is necessary and a default cannot be provided because any name defined in
/// the macro is invisible to the calling code. In the example, the name `x` is
/// selected.
///
/// Then a name and optional visibility is specified for the entry point. This
/// will be a zero-size marker struct implementing [ExternFn]. It can also have
/// documentation and attributes.
///
/// This is followed by the table of arguments. Each defines a name, value type,
/// and a conversion expression which references the [ExprInst] by the name
/// defined in the first step and returns a [Result] of the success type or
/// `Rc<dyn ExternError>`.
///
/// To avoid typing the same expression a lot, the conversion is optional.
/// If it is omitted, the field is initialized with a [TryInto::try_into] call
/// from `&ExprInst` to the target type. In this case, the error is
/// short-circuited using `?` so conversions through `FromResidual` are allowed.
/// The optional syntax starts with `as`.
///
/// If all conversions are omitted, the alias definition (`expr=$ident in`) has
/// no effect and is therefore optional.
///
/// Finally, the body of the function is provided as an expression which can
/// reference all of the arguments by their names, each bound to a ref of the
/// specified type.
///
/// ```
/// use orchidlang::interpreted::Clause;
/// use orchidlang::systems::cast_exprinst::with_str;
/// use orchidlang::{define_fn, Literal, OrcString, Primitive};
///
/// define_fn! {expr=x in
///   /// Append a string to another
///   pub Concatenate {
///     a: OrcString as with_str(x, |s| Ok(s.clone())),
///     b: OrcString as with_str(x, |s| Ok(s.clone()))
///   } => {
///     Ok(Clause::P(Primitive::Literal(Literal::Str(
///       OrcString::from(a.get_string() + &b)
///     ))))
///   }
/// }
/// ```
///
/// A simpler format is also offered for unary functions:
///
/// ```
/// use orchidlang::interpreted::Clause;
/// use orchidlang::systems::cast_exprinst::with_lit;
/// use orchidlang::{define_fn, Literal};
///
/// define_fn! {
///   /// Convert a literal to a string using Rust's conversions for floats,
///   /// chars and uints respectively
///   ToString = |x| with_lit(x, |l| Ok(match l {
///     Literal::Uint(i) => Literal::Str(i.to_string().into()),
///     Literal::Num(n) => Literal::Str(n.to_string().into()),
///     s@Literal::Str(_) => s.clone(),
///   })).map(Clause::from)
/// }
/// ```
#[macro_export]
macro_rules! define_fn {
  // Unary function entry
  ($( #[ $attr:meta ] )* $qual:vis $name:ident = |$x:ident| $body:expr) => {
    paste::paste!{
      $crate::write_fn_step!(
        $( #[ $attr ] )* $qual $name
        >
        [< Internal $name >]
      );
      $crate::write_fn_step!(
        [< Internal $name >]
        {}
        out = expr => Ok(expr);
        {
          let lambda = |$x: &$crate::interpreted::ExprInst| $body;
          lambda(out)
        }
      );
    }
  };
  // xname is optional only if every conversion is implicit
  ($( #[ $attr:meta ] )* $qual:vis $name:ident {
    $( $arg:ident: $typ:ty ),+  $(,)?
  } => $body:expr) => {
    $crate::define_fn!{expr=expr in
      $( #[ $attr ] )* $qual $name {
        $( $arg: $typ ),*
      } => $body
    }
  };
  // multi-parameter function entry
  (expr=$xname:ident in
    $( #[ $attr:meta ] )*
    $qual:vis $name:ident {
      $arg0:ident: $typ0:ty $( as $parse0:expr )?
      $(, $arg:ident: $typ:ty $( as $parse:expr )? )* $(,)?
    } => $body:expr
  ) => {paste::paste!{
    // Generate initial state
    $crate::write_fn_step!(
      $( #[ $attr ] )* $qual $name
      >
      [< Internal $name >]
    );
    // Enter loop to generate intermediate states
    $crate::define_fn!(@MIDDLE $xname [< Internal $name >] ($body)
      ()
      (
        ( $arg0: $typ0 $( as $parse0)? )
        $(
          ( $arg: $typ $( as $parse)? )
        )*
      )
    );
  }};
  // Recursive case
  (@MIDDLE $xname:ident $name:ident ($body:expr)
    // fields that should be included in this struct
    (
      $(
        ( $arg_prev:ident: $typ_prev:ty )
      )*
    )
    // later fields
    (
      // field that should be processed by this step
      ( $arg0:ident: $typ0:ty $( as $parse0:expr )? )
      // ensure that we have a next stage
      $(
        ( $arg:ident: $typ:ty $( as $parse:expr )? )
      )+
    )
  ) => {paste::paste!{
    $crate::write_fn_step!(
      $name
      {
        $( $arg_prev : $typ_prev ),*
      }
      [< $name $arg0:camel >]
      where $arg0:$typ0 $( = $xname => $parse0 )? ;
    );
    $crate::define_fn!(@MIDDLE $xname [< $name $arg0:camel >] ($body)
      (
        $( ($arg_prev: $typ_prev) )*
        ($arg0: $typ0)
      )
      (
        $(
          ( $arg: $typ $( as $parse)? )
        )+
      )
    );
  }};
  // recursive base case
  (@MIDDLE $xname:ident $name:ident ($body:expr)
    // all but one field is included in this struct
    (
      $( ($arg_prev:ident: $typ_prev:ty) )*
    )
    // the last one is initialized before the body runs
    (
      ($arg0:ident: $typ0:ty $( as $parse0:expr )? )
    )
  ) => {
    $crate::write_fn_step!(
      $name
      {
        $( $arg_prev: $typ_prev ),*
      }
      $arg0:$typ0 $( = $xname => $parse0 )? ;
      $body
    );
  };
}
