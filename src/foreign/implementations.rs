use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

use super::atom::{Atom, Atomic, AtomicResult};
use super::error::{ExternError, ExternResult};
use super::process::Unstable;
use super::to_clause::ToClause;
use crate::gen::tpl;
use crate::gen::traits::Gen;
use crate::interpreter::apply::CallData;
use crate::interpreter::error::RunError;
use crate::interpreter::gen_nort::nort_gen;
use crate::interpreter::nort::{Clause, ClauseInst};
use crate::interpreter::run::RunData;
use crate::location::CodeLocation;
use crate::utils::clonable_iter::Clonable;
use crate::utils::ddispatch::Responder;

impl<T: ToClause> ToClause for Option<T> {
  fn to_clause(self, location: CodeLocation) -> Clause {
    let ctx = nort_gen(location.clone());
    match self {
      None => tpl::C("std::option::none").template(ctx, []),
      Some(t) => tpl::A(tpl::C("std::option::some"), tpl::Slot)
        .template(ctx, [t.to_clause(location)]),
    }
  }
}

impl<T: ToClause, U: ToClause> ToClause for Result<T, U> {
  fn to_clause(self, location: CodeLocation) -> Clause {
    let ctx = nort_gen(location.clone());
    match self {
      Ok(t) => tpl::A(tpl::C("std::result::ok"), tpl::Slot)
        .template(ctx, [t.to_clause(location)]),
      Err(e) => tpl::A(tpl::C("std::result::err"), tpl::Slot)
        .template(ctx, [e.to_clause(location)]),
    }
  }
}

struct PendingError(Arc<dyn ExternError>);
impl Responder for PendingError {}
impl Debug for PendingError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "PendingError({})", self.0)
  }
}
impl Atomic for PendingError {
  fn as_any(self: Box<Self>) -> Box<dyn Any> { self }
  fn as_any_ref(&self) -> &dyn Any { self }
  fn redirect(&mut self) -> Option<&mut ClauseInst> { None }
  fn run(self: Box<Self>, _: RunData) -> AtomicResult {
    Err(RunError::Extern(self.0))
  }
  fn apply_ref(&self, _: CallData) -> ExternResult<Clause> {
    panic!("This atom decays instantly")
  }
}

impl<T: ToClause> ToClause for ExternResult<T> {
  fn to_clause(self, location: CodeLocation) -> Clause {
    match self {
      Err(e) => PendingError(e).atom_cls(),
      Ok(t) => t.to_clause(location),
    }
  }
}

struct ListGen<I>(Clonable<I>)
where
  I: Iterator + Send,
  I::Item: ToClause + Send;
impl<I> Clone for ListGen<I>
where
  I: Iterator + Send,
  I::Item: ToClause + Send,
{
  fn clone(&self) -> Self { Self(self.0.clone()) }
}
impl<I> ToClause for ListGen<I>
where
  I: Iterator + Send + 'static,
  I::Item: ToClause + Clone + Send,
{
  fn to_clause(mut self, location: CodeLocation) -> Clause {
    let ctx = nort_gen(location.clone());
    match self.0.next() {
      None => tpl::C("std::lit::end").template(ctx, []),
      Some(val) => {
        let atom = Unstable::new(|run| self.to_clause(run.location));
        tpl::a2(tpl::C("std::lit::cons"), tpl::Slot, tpl::V(atom))
          .template(ctx, [val.to_clause(location)])
      },
    }
  }
}

/// Convert an iterator into a lazy-evaluated Orchid list.
pub fn list<I>(items: I) -> impl ToClause
where
  I: IntoIterator + Clone + Send + Sync + 'static,
  I::IntoIter: Send,
  I::Item: ToClause + Clone + Send,
{
  Unstable::new(move |RunData { location, .. }| {
    ListGen(Clonable::new(
      items.clone().into_iter().map(move |t| t.to_clsi(location.clone())),
    ))
  })
}

impl<T: ToClause + Clone + Send + Sync + 'static> ToClause for Vec<T> {
  fn to_clause(self, location: CodeLocation) -> Clause {
    list(self).to_clause(location)
  }
}

impl ToClause for Atom {
  fn to_clause(self, _: CodeLocation) -> Clause { Clause::Atom(self) }
}

mod tuple_impls {
  use std::sync::Arc;

  use super::ToClause;
  use crate::foreign::atom::Atomic;
  use crate::foreign::error::AssertionError;
  use crate::foreign::implementations::ExternResult;
  use crate::foreign::inert::Inert;
  use crate::foreign::try_from_expr::TryFromExpr;
  use crate::interpreter::nort::{Clause, Expr};
  use crate::libs::std::tuple::Tuple;
  use crate::location::CodeLocation;

  macro_rules! gen_tuple_impl {
    ( ($($T:ident)*) ($($t:ident)*)) => {
      impl<$($T: ToClause),*> ToClause for ($($T,)*) {
        fn to_clause(self, location: CodeLocation) -> Clause {
          let ($($t,)*) = self;
          Inert(Tuple(Arc::new(vec![
            $($t.to_expr(location.clone()),)*
          ]))).atom_cls()
        }
      }

      impl<$($T: TryFromExpr),*> TryFromExpr for ($($T,)*) {
        fn from_expr(ex: Expr) -> ExternResult<Self> {
          let Inert(Tuple(slice)) = ex.clone().downcast()?;
          match &slice[..] {
            [$($t),*] => Ok(($($t.clone().downcast()?,)*)),
            _ => AssertionError::fail(ex.location(), "Tuple length mismatch", format!("{ex}"))
          }
        }
      }
    };
  }

  gen_tuple_impl!((A)(a));
  gen_tuple_impl!((A B) (a b));
  gen_tuple_impl!((A B C) (a b c));
  gen_tuple_impl!((A B C D) (a b c d));
  gen_tuple_impl!((A B C D E) (a b c d e));
  gen_tuple_impl!((A B C D E F) (a b c d e f));
  gen_tuple_impl!((A B C D E F G) (a b c d e f g));
  gen_tuple_impl!((A B C D E F G H) (a b c d e f g h));
  gen_tuple_impl!((A B C D E F G H I) (a b c d e f g h i));
  gen_tuple_impl!((A B C D E F G H I J) (a b c d e f g h i j));
  gen_tuple_impl!((A B C D E F G H I J K) (a b c d e f g h i j k));
  gen_tuple_impl!((A B C D E F G H I J K L) (a b c d e f g h i j k l));
}
