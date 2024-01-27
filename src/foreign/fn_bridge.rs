use std::any::{Any, TypeId};
use std::fmt::Debug;
use std::marker::PhantomData;

use super::atom::{Atomic, AtomicResult, AtomicReturn};
use super::error::ExternResult;
use super::to_clause::ToClause;
use super::try_from_expr::TryFromExpr;
use crate::interpreter::apply::CallData;
use crate::interpreter::context::Halt;
use crate::interpreter::nort::{Clause, ClauseInst, Expr};
use crate::interpreter::run::{run, RunData};
use crate::utils::ddispatch::Responder;

/// Return a unary lambda wrapped in this struct to take an additional argument
/// in a function passed to Orchid through a member of the [super::xfn_1ary]
/// family.
///
/// Container for a unary [FnOnce] that uniquely states the argument and return
/// type. Rust functions are never overloaded, but inexplicably the [Fn] traits
/// take the argument tuple as a generic parameter which means that it cannot
/// be a unique dispatch target.
///
/// If the function takes an instance of [Lazy], it will contain the expression
/// the function was applied to without any specific normalization. If it takes
/// any other type, the argument will be fully normalized and cast using the
/// type's [TryFromExpr] impl.
pub struct Param<T, U, F> {
  data: F,
  _t: PhantomData<T>,
  _u: PhantomData<U>,
}
unsafe impl<T, U, F: Send> Send for Param<T, U, F> {}
impl<T, U, F> Param<T, U, F> {
  /// Wrap a new function in a parametric struct
  pub fn new(f: F) -> Self
  where F: FnOnce(T) -> U {
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

/// A marker struct that gets assigned an expression without normalizing it.
/// This behaviour cannot be replicated in usercode, it's implemented with an
/// explicit runtime [TypeId] check invoked by [Param].
#[derive(Debug, Clone)]
pub struct Thunk(pub Expr);
impl TryFromExpr for Thunk {
  fn from_expr(expr: Expr) -> ExternResult<Self> { Ok(Thunk(expr)) }
}

struct FnMiddleStage<T, U, F> {
  arg: Expr,
  f: Param<T, U, F>,
}

impl<T, U, F: Clone> Clone for FnMiddleStage<T, U, F> {
  fn clone(&self) -> Self { Self { arg: self.arg.clone(), f: self.f.clone() } }
}
impl<T, U, F> Debug for FnMiddleStage<T, U, F> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("FnMiddleStage")
      .field("argument", &self.arg)
      .finish_non_exhaustive()
  }
}
impl<T, U, F> Responder for FnMiddleStage<T, U, F> {}
impl<
  T: 'static + TryFromExpr,
  U: 'static + ToClause,
  F: 'static + Clone + FnOnce(T) -> U + Any + Send,
> Atomic for FnMiddleStage<T, U, F>
{
  fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> { self }
  fn as_any_ref(&self) -> &dyn std::any::Any { self }
  fn redirect(&mut self) -> Option<&mut ClauseInst> {
    // this should be ctfe'd
    (TypeId::of::<T>() != TypeId::of::<Thunk>()).then(|| &mut self.arg.clause)
  }
  fn run(self: Box<Self>, r: RunData) -> AtomicResult {
    let Self { arg, f: Param { data: f, .. } } = *self;
    let clause = f(arg.downcast()?).to_clause(r.location);
    Ok(AtomicReturn { gas: r.ctx.gas, inert: false, clause })
  }
  fn apply_ref(&self, _: CallData) -> ExternResult<Clause> {
    panic!("Atom should have decayed")
  }
}

impl<T, U, F> Responder for Param<T, U, F> {}
impl<T, U, F> Debug for Param<T, U, F> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Param")
  }
}

impl<
  T: 'static + TryFromExpr + Clone,
  U: 'static + ToClause,
  F: 'static + Clone + Send + FnOnce(T) -> U,
> Atomic for Param<T, U, F>
{
  fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> { self }
  fn as_any_ref(&self) -> &dyn std::any::Any { self }
  fn redirect(&mut self) -> Option<&mut ClauseInst> { None }
  fn run(self: Box<Self>, r: RunData) -> AtomicResult {
    AtomicReturn::inert(*self, r.ctx)
  }
  fn apply_ref(&self, call: CallData) -> ExternResult<Clause> {
    Ok(FnMiddleStage { arg: call.arg, f: self.clone() }.atom_cls())
  }
  fn apply(self: Box<Self>, call: CallData) -> ExternResult<Clause> {
    Ok(FnMiddleStage { arg: call.arg, f: *self }.atom_cls())
  }
}

/// Conversion functions from [Fn] traits into [Atomic]. Since Rust's type
/// system allows overloaded [Fn] implementations, we must specify the arity and
/// argument types for this process. Arities are only defined up to 9, but the
/// function can always return another call to `xfn_`N`ary` to consume more
/// arguments.
pub mod constructors {
  use super::super::atom::Atomic;
  use super::super::try_from_expr::TryFromExpr;
  #[allow(unused)] // for doc
  use super::Thunk;
  use super::{Param, ToClause};

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
          "- All arguments must implement [TryFromExpr]\n"
          "- all but the last argument must implement [Clone] and [Send]\n"
          "- the return type must implement [ToClause].\n\n"
        ]
        #[doc = "Take [Lazy] to take the argument as-is,\n"
          "without normalization\n\n"
        ]
        #[doc = "Other arities: " $( "[xfn_" $alt "ary], " )+ ]
        pub fn [< xfn_ $number ary >] <
          $( $t : TryFromExpr + Clone + Send + 'static, )*
          TLast: TryFromExpr + Clone + 'static,
          TReturn: ToClause + Send + 'static,
          TFunction: FnOnce( $( $t , )* TLast )
            -> TReturn + Clone + Send + 'static
        >(function: TFunction) -> impl Atomic + Clone {
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
        xfn_variant!(@BODY_LOOP $function ( $( ( $T $t ) )* ) $full)
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
