use std::fmt::Debug;
use std::marker::PhantomData;
use std::rc::Rc;

use super::atom::StrictEq;
use super::{
  Atomic, AtomicResult, AtomicReturn, ExternError, ExternFn, XfnResult,
};
use crate::ddispatch::Responder;
use crate::interpreted::{Clause, ExprInst, TryFromExprInst};
use crate::interpreter::{run, Context, Return};
use crate::systems::codegen::{opt, res};
use crate::OrcString;

/// A trait for things that are infallibly convertible to [Clause]. These types
/// can be returned by callbacks passed to the [super::xfn_1ary] family of
/// functions.
pub trait ToClause: Clone {
  /// Convert the type to a [Clause].
  fn to_clause(self) -> Clause;
  /// Convert to an expression instance via [ToClause].
  fn to_exi(self) -> ExprInst { self.to_clause().wrap() }
}

impl<T: Atomic + Clone> ToClause for T {
  fn to_clause(self) -> Clause { self.atom_cls() }
}
impl ToClause for Clause {
  fn to_clause(self) -> Clause { self }
}
impl ToClause for ExprInst {
  fn to_clause(self) -> Clause { self.expr_val().clause }
}
impl ToClause for String {
  fn to_clause(self) -> Clause { OrcString::from(self).atom_cls() }
}
impl<T: ToClause> ToClause for Option<T> {
  fn to_clause(self) -> Clause { opt(self.map(|t| t.to_clause().wrap())) }
}
impl<T: ToClause, U: ToClause> ToClause for Result<T, U> {
  fn to_clause(self) -> Clause {
    res(self.map(|t| t.to_clause().wrap()).map_err(|u| u.to_clause().wrap()))
  }
}

/// Return a unary lambda wrapped in this struct to take an additional argument
/// in a function passed to Orchid through a member of the [super::xfn_1ary]
/// family.
///
/// Container for a unary [FnOnce] that uniquely states the argument and return
/// type. Rust functions are never overloaded, but inexplicably the [Fn] traits
/// take the argument tuple as a generic parameter which means that it cannot
/// be a unique dispatch target.
pub struct Param<T, U, F> {
  data: F,
  _t: PhantomData<T>,
  _u: PhantomData<U>,
}
unsafe impl<T, U, F: Send> Send for Param<T, U, F> {}
impl<T, U, F> Param<T, U, F> {
  /// Wrap a new function in a parametric struct
  pub fn new(f: F) -> Self
  where
    F: FnOnce(T) -> Result<U, Rc<dyn ExternError>>,
  {
    Self { data: f, _t: PhantomData, _u: PhantomData }
  }
  /// Take out the function
  pub fn get(self) -> F { self.data }
}
impl<T, U, F: Clone> Clone for Param<T, U, F> {
  fn clone(&self) -> Self {
    Self { data: self.data.clone(), _t: PhantomData, _u: PhantomData }
  }
}

impl<
  T: 'static + TryFromExprInst,
  U: 'static + ToClause,
  F: 'static + Clone + Send + FnOnce(T) -> Result<U, Rc<dyn ExternError>>,
> ToClause for Param<T, U, F>
{
  fn to_clause(self) -> Clause { self.xfn_cls() }
}

struct FnMiddleStage<T, U, F> {
  argument: ExprInst,
  f: Param<T, U, F>,
}
impl<T, U, F> StrictEq for FnMiddleStage<T, U, F> {
  fn strict_eq(&self, _other: &dyn std::any::Any) -> bool {
    unimplemented!("This should never be able to appear in a pattern")
  }
}

impl<T, U, F: Clone> Clone for FnMiddleStage<T, U, F> {
  fn clone(&self) -> Self {
    Self { argument: self.argument.clone(), f: self.f.clone() }
  }
}
impl<T, U, F> Debug for FnMiddleStage<T, U, F> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("FnMiddleStage")
      .field("argument", &self.argument)
      .finish_non_exhaustive()
  }
}
impl<T, U, F> Responder for FnMiddleStage<T, U, F> {}
impl<
  T: 'static + TryFromExprInst,
  U: 'static + ToClause,
  F: 'static + Clone + FnOnce(T) -> Result<U, Rc<dyn ExternError>> + Send,
> Atomic for FnMiddleStage<T, U, F>
{
  fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> { self }
  fn as_any_ref(&self) -> &dyn std::any::Any { self }
  fn run(self: Box<Self>, ctx: Context) -> AtomicResult {
    let Return { gas, inert, state } = run(self.argument, ctx)?;
    let clause = match inert {
      false => state.expr_val().clause,
      true => (self.f.data)(state.downcast()?)?.to_clause(),
    };
    Ok(AtomicReturn { gas, inert: false, clause })
  }
}

impl<
  T: 'static + TryFromExprInst,
  U: 'static + ToClause,
  F: 'static + Clone + Send + FnOnce(T) -> Result<U, Rc<dyn ExternError>>,
> ExternFn for Param<T, U, F>
{
  fn name(&self) -> &str { "anonymous Rust function" }
  fn apply(self: Box<Self>, arg: ExprInst, _: Context) -> XfnResult<Clause> {
    Ok(FnMiddleStage { argument: arg, f: *self }.atom_cls())
  }
}

pub mod constructors {
  use std::rc::Rc;

  use super::{Param, ToClause};
  use crate::foreign::{ExternError, ExternFn};
  use crate::interpreted::TryFromExprInst;

  macro_rules! xfn_variant {
    (
      $number:expr,
      ($($t:ident)*)
      ($($alt:expr)*)
    ) => {
      paste::paste!{
        #[doc = "Convert a function of " $number " argument(s) into a curried"
          " Orchid function. See also Constraints summarized:\n\n"
          "- the callback must live as long as `'static`\n"
          "- All arguments must implement [TryFromExprInst]\n"
          "- all but the last argument must implement [Clone] and [Send]\n"
          "- the return type must implement [ToClause].\n\n"
        ]
        #[doc = "Other arities: " $( "[xfn_" $alt "ary], " )+ ]
        pub fn [< xfn_ $number ary >] <
          $( $t : TryFromExprInst + Clone + Send + 'static, )*
          TLast: TryFromExprInst + 'static,
          TReturn: ToClause + Send + 'static,
          TFunction: FnOnce( $( $t , )* TLast )
            -> Result<TReturn, Rc<dyn ExternError>> + Clone + Send + 'static
        >(function: TFunction) -> impl ExternFn {
          xfn_variant!(@BODY_LOOP function
            ( $( ( $t [< $t:lower >] ) )* )
            ( $( [< $t:lower >] )* )
          )
        }
      }
    };
    (@BODY_LOOP $function:ident (
      ( $Next:ident $next:ident )
      $( ( $T:ident $t:ident ) )*
    ) $full:tt) => {
      Param::new(|$next : $Next| {
        Ok(xfn_variant!(@BODY_LOOP $function ( $( ( $T $t ) )* ) $full))
      })
    };
    (@BODY_LOOP $function:ident () ( $( $t:ident )* )) => {
      Param::new(|last: TLast| $function ( $( $t , )* last ))
    };
  }

  xfn_variant!(1, () (2 3 4 5 6 7 8 9 10 11 12 13 14 15 16));
  xfn_variant!(2, (A) (1 3 4 5 6 7 8 9 10 11 12 13 14 15 16));
  xfn_variant!(3, (A B) (1 2 4 5 6 7 8 9 10 11 12 13 14 15 16));
  xfn_variant!(4, (A B C) (1 2 3 5 6 7 8 9 10 11 12 13 14 15 16));
  xfn_variant!(5, (A B C D) (1 2 3 4 6 7 8 9 10 11 12 13 14 15 16));
  xfn_variant!(6, (A B C D E) (1 2 3 4 5 7 8 9 10 11 12 13 14 15 16));
  xfn_variant!(7, (A B C D E F) (1 2 3 4 5 6 8 9 10 11 12 13 14 15 16));
  xfn_variant!(8, (A B C D E F G) (1 2 3 4 5 6 7 9 10 11 12 13 14 15 16));
  xfn_variant!(9, (A B C D E F G H) (1 2 3 4 5 6 7 8 10 11 12 13 14 15 16));
  // at higher arities rust-analyzer fails to load the project
}
