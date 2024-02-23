//! Conversions from Rust values to Orchid expressions. Many APIs and
//! [super::fn_bridge] in particular use this to automatically convert values on
//! the boundary. The opposite conversion is [super::try_from_expr::TryFromExpr]

use super::atom::{Atomic, RunData};
use super::process::Unstable;
use crate::gen::tpl;
use crate::gen::traits::Gen;
use crate::interpreter::gen_nort::nort_gen;
use crate::interpreter::nort::{Clause, ClauseInst, Expr};
use crate::location::CodeLocation;
use crate::utils::clonable_iter::Clonable;

/// A trait for things that are infallibly convertible to [ClauseInst]. These
/// types can be returned by callbacks passed to [super::fn_bridge::xfn].
pub trait ToClause: Sized {
  /// Convert this value to a [Clause]. If your value can only be directly
  /// converted to a [ClauseInst], you can call `ClauseInst::to_clause` to
  /// unwrap it if possible or fall back to [Clause::Identity].
  fn to_clause(self, location: CodeLocation) -> Clause;

  /// Convert the type to a [Clause].
  fn to_clsi(self, location: CodeLocation) -> ClauseInst {
    ClauseInst::new(self.to_clause(location))
  }

  /// Convert to an expression via [ToClause].
  fn to_expr(self, location: CodeLocation) -> Expr {
    Expr { clause: self.to_clsi(location.clone()), location }
  }
}

impl<T: Atomic + Clone> ToClause for T {
  fn to_clause(self, _: CodeLocation) -> Clause { self.atom_cls() }
}
impl ToClause for Clause {
  fn to_clause(self, _: CodeLocation) -> Clause { self }
}
impl ToClause for ClauseInst {
  fn to_clause(self, _: CodeLocation) -> Clause { self.into_cls() }
  fn to_clsi(self, _: CodeLocation) -> ClauseInst { self }
}
impl ToClause for Expr {
  fn to_clause(self, location: CodeLocation) -> Clause { self.clause.to_clause(location) }
  fn to_clsi(self, _: CodeLocation) -> ClauseInst { self.clause }
  fn to_expr(self, _: CodeLocation) -> Expr { self }
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
      None => tpl::C("std::list::end").template(ctx, []),
      Some(val) => {
        let atom = Unstable::new(|run| self.to_clause(run.location));
        tpl::a2(tpl::C("std::list::cons"), tpl::Slot, tpl::V(atom))
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
    ListGen(Clonable::new(items.clone().into_iter().map(move |t| t.to_clsi(location.clone()))))
  })
}

mod implementations {
  use std::any::Any;
  use std::fmt;
  use std::sync::Arc;

  use super::{list, ToClause};
  use crate::foreign::atom::{Atom, Atomic, AtomicResult, CallData, RunData};
  use crate::foreign::error::{AssertionError, RTErrorObj, RTResult};
  use crate::foreign::inert::Inert;
  use crate::foreign::try_from_expr::TryFromExpr;
  use crate::gen::tpl;
  use crate::gen::traits::Gen;
  use crate::interpreter::gen_nort::nort_gen;
  use crate::interpreter::nort::{Clause, Expr};
  use crate::libs::std::tuple::Tuple;
  use crate::location::CodeLocation;
  use crate::utils::ddispatch::Responder;

  impl<T: ToClause> ToClause for Option<T> {
    fn to_clause(self, location: CodeLocation) -> Clause {
      let ctx = nort_gen(location.clone());
      match self {
        None => tpl::C("std::option::none").template(ctx, []),
        Some(t) =>
          tpl::A(tpl::C("std::option::some"), tpl::Slot).template(ctx, [t.to_clause(location)]),
      }
    }
  }

  impl<T: ToClause, U: ToClause> ToClause for Result<T, U> {
    fn to_clause(self, location: CodeLocation) -> Clause {
      let ctx = nort_gen(location.clone());
      match self {
        Ok(t) =>
          tpl::A(tpl::C("std::result::ok"), tpl::Slot).template(ctx, [t.to_clause(location)]),
        Err(e) =>
          tpl::A(tpl::C("std::result::err"), tpl::Slot).template(ctx, [e.to_clause(location)]),
      }
    }
  }

  struct PendingError(RTErrorObj);
  impl Responder for PendingError {}
  impl fmt::Debug for PendingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "PendingError({})", self.0)
    }
  }
  impl Atomic for PendingError {
    fn as_any(self: Box<Self>) -> Box<dyn Any> { self }
    fn as_any_ref(&self) -> &dyn Any { self }
    fn type_name(&self) -> &'static str { std::any::type_name::<Self>() }

    fn redirect(&mut self) -> Option<&mut Expr> { None }
    fn run(self: Box<Self>, _: RunData) -> AtomicResult { Err(self.0) }
    fn apply_mut(&mut self, _: CallData) -> RTResult<Clause> {
      panic!("This atom decays instantly")
    }
  }

  impl<T: ToClause> ToClause for RTResult<T> {
    fn to_clause(self, location: CodeLocation) -> Clause {
      match self {
        Err(e) => PendingError(e).atom_cls(),
        Ok(t) => t.to_clause(location),
      }
    }
  }

  impl<T: ToClause + Clone + Send + Sync + 'static> ToClause for Vec<T> {
    fn to_clause(self, location: CodeLocation) -> Clause { list(self).to_clause(location) }
  }

  impl ToClause for Atom {
    fn to_clause(self, _: CodeLocation) -> Clause { Clause::Atom(self) }
  }

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
        fn from_expr(ex: Expr) -> RTResult<Self> {
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
