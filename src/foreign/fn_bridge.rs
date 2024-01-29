use std::any::{Any, TypeId};
use std::fmt::{Debug, Display};
use std::marker::PhantomData;

use intern_all::{i, Tok};

use super::atom::{Atomic, AtomicResult, AtomicReturn, CallData, RunData};
use super::error::ExternResult;
use super::to_clause::ToClause;
use super::try_from_expr::TryFromExpr;
use crate::interpreter::nort::{Clause, Expr};
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
  name: Tok<String>,
  _t: PhantomData<T>,
  _u: PhantomData<U>,
}
unsafe impl<T, U, F: Send> Send for Param<T, U, F> {}
impl<T, U, F> Param<T, U, F> {
  /// Wrap a new function in a parametric struct
  pub fn new(name: Tok<String>, f: F) -> Self
  where F: FnOnce(T) -> U {
    Self { name, data: f, _t: PhantomData, _u: PhantomData }
  }
  /// Take out the function
  pub fn get(self) -> F { self.data }
}
impl<T, U, F: Clone> Clone for Param<T, U, F> {
  fn clone(&self) -> Self {
    Self { name: self.name.clone(), data: self.data.clone(), _t: PhantomData, _u: PhantomData }
  }
}
impl<T, U, F> Display for Param<T, U, F> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.name) }
}
impl<T, U, F> Debug for Param<T, U, F> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("Param").field(&*self.name).finish()
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
    write!(f, "FnMiddleStage({} {})", self.f, self.arg)
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
  fn redirect(&mut self) -> Option<&mut Expr> {
    // this should be ctfe'd
    (TypeId::of::<T>() != TypeId::of::<Thunk>()).then_some(&mut self.arg)
  }
  fn run(self: Box<Self>, r: RunData) -> AtomicResult {
    let Self { arg, f: Param { data: f, .. } } = *self;
    Ok(AtomicReturn::Change(0, f(arg.downcast()?).to_clause(r.location)))
  }
  fn apply_ref(&self, _: CallData) -> ExternResult<Clause> { panic!("Atom should have decayed") }
}

impl<T, U, F> Responder for Param<T, U, F> {}

impl<
  T: 'static + TryFromExpr + Clone,
  U: 'static + ToClause,
  F: 'static + Clone + Send + FnOnce(T) -> U,
> Atomic for Param<T, U, F>
{
  fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> { self }
  fn as_any_ref(&self) -> &dyn std::any::Any { self }
  fn redirect(&mut self) -> Option<&mut Expr> { None }
  fn run(self: Box<Self>, _: RunData) -> AtomicResult { AtomicReturn::inert(*self) }
  fn apply_ref(&self, call: CallData) -> ExternResult<Clause> {
    Ok(FnMiddleStage { arg: call.arg, f: self.clone() }.atom_cls())
  }
  fn apply(self: Box<Self>, call: CallData) -> ExternResult<Clause> {
    Ok(FnMiddleStage { arg: call.arg, f: *self }.atom_cls())
  }
}

/// Convert a Rust function to Orchid. If you can, register your Rust functions
/// statically with functions in [crate::gen::tree].
pub fn xfn<const N: usize, Argv, Ret>(
  name: &str,
  x: impl Xfn<N, Argv, Ret>,
) -> impl Atomic + Clone {
  x.to_atomic(i(name))
}

/// Trait for functions that can be directly passed to Orchid. Constraints in a
/// nutshell:
///
/// - the function must live as long as ['static]
/// - All arguments must implement [TryFromExpr]
/// - all but the last argument must implement [Clone] and [Send]
/// - the return type must implement [ToClause]
///
/// Take [Thunk] to consume the argument as-is, without normalization.
pub trait Xfn<const N: usize, Argv, Ret>: Clone + 'static {
  /// Convert Rust type to Orchid function, given a name for logging
  fn to_atomic(self, name: Tok<String>) -> impl Atomic + Clone;
}

/// Conversion functions from [Fn] traits into [Atomic]. Since Rust's type
/// system allows overloaded [Fn] implementations, we must specify the arity and
/// argument types for this process. Arities are only defined up to 9, but the
/// function can always return another call to `xfn_`N`ary` to consume more
/// arguments.
pub mod xfn_impls {
  use intern_all::{i, Tok};

  use super::super::atom::Atomic;
  use super::super::try_from_expr::TryFromExpr;
  #[allow(unused)] // for doc
  use super::Thunk;
  use super::{Param, ToClause, Xfn};

  macro_rules! xfn_variant {
    (
      $number:expr,
      ($($t:ident)*)
      ($($alt:expr)*)
    ) => {
      paste::paste!{
        impl<
          $( $t : TryFromExpr + Clone + Send + 'static, )*
          TLast: TryFromExpr + Clone + 'static,
          TReturn: ToClause + Send + 'static,
          TFunction: FnOnce( $( $t , )* TLast )
            -> TReturn + Clone + Send + 'static
        > Xfn<$number, ($($t,)* TLast,), TReturn> for TFunction {
          fn to_atomic(self, name: Tok<String>) -> impl Atomic + Clone {
            #[allow(unused_variables)]
            let argc = 0;
            let stage_n = name.clone();
            xfn_variant!(@BODY_LOOP self name stage_n argc
              ( $( ( $t [< $t:lower >] ) )* )
              ( $( [< $t:lower >] )* )
            )
          }
        }
      }
    };
    (@BODY_LOOP $function:ident $name:ident $stage_n:ident $argc:ident (
      ( $Next:ident $next:ident )
      $( ( $T:ident $t:ident ) )*
    ) $full:tt) => {{
      Param::new($stage_n, move |$next : $Next| {
        let $argc = $argc + 1;
        let $stage_n = i(&format!("{}/{}", $name, $argc));
        xfn_variant!(@BODY_LOOP $function $name $stage_n $argc ( $( ( $T $t ) )* ) $full)
      })
    }};
    (@BODY_LOOP $function:ident $name:ident $stage_n:ident $argc:ident (

    ) ( $( $t:ident )* )) => {{
      Param::new($stage_n, |last: TLast| $function ( $( $t , )* last ))
    }};
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
