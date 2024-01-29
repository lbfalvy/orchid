use std::collections::VecDeque;
use std::iter;
use std::rc::Rc;

use crate::foreign::atom::Atomic;
use crate::foreign::fn_bridge::xfn;
use crate::foreign::process::Unstable;
use crate::foreign::to_clause::ToClause;
use crate::foreign::try_from_expr::TryFromExpr;
use crate::location::{CodeLocation, SourceRange};
use crate::parse::parsed::{self, PType};
use crate::utils::pure_seq::pushed;

pub trait DeferredRuntimeCallback<T, U, R: ToClause>:
  Fn(Vec<(T, U)>) -> R + Clone + Send + 'static
{
}
impl<T, U, R: ToClause, F: Fn(Vec<(T, U)>) -> R + Clone + Send + 'static>
  DeferredRuntimeCallback<T, U, R> for F
{
}

/// Lazy-recursive function that takes the next value from the interpreter
/// and acts upon it
///
/// # Panics
///
/// If the list of remaining keys is empty
fn table_receiver_rec<
  T: Clone + Send + 'static,
  U: TryFromExpr + Clone + Send + 'static,
  R: ToClause + 'static,
>(
  range: SourceRange,
  results: Vec<(T, U)>,
  mut remaining_keys: VecDeque<T>,
  callback: impl DeferredRuntimeCallback<T, U, R>,
) -> impl Atomic {
  let t = remaining_keys.pop_front().expect("empty handled elsewhere");
  xfn("__table_receiver__", move |u: U| {
    let results = pushed(results, (t, u));
    match remaining_keys.is_empty() {
      true => callback(results).to_clause(CodeLocation::Source(range)),
      false => table_receiver_rec(range, results, remaining_keys, callback).atom_cls(),
    }
  })
}

fn table_receiver<
  T: Clone + Send + 'static,
  U: TryFromExpr + Clone + Send + 'static,
  R: ToClause + 'static,
>(
  range: SourceRange,
  keys: VecDeque<T>,
  callback: impl DeferredRuntimeCallback<T, U, R>,
) -> parsed::Clause {
  if keys.is_empty() {
    Unstable::new(move |_| callback(Vec::new())).ast_cls()
  } else {
    Unstable::new(move |_| table_receiver_rec(range, Vec::new(), keys, callback).atom_cls())
      .ast_cls()
  }
}

/// Defers the execution of the callback to runtime, allowing it to depend on
/// the result of Otchid expressions.
pub fn defer_to_runtime<
  T: Clone + Send + 'static,
  U: TryFromExpr + Clone + Send + 'static,
  R: ToClause + 'static,
>(
  range: SourceRange,
  pairs: impl IntoIterator<Item = (T, Vec<parsed::Expr>)>,
  callback: impl DeferredRuntimeCallback<T, U, R>,
) -> parsed::Clause {
  let (keys, ast_values) = pairs.into_iter().unzip::<_, _, VecDeque<_>, Vec<_>>();
  let items = iter::once(table_receiver(range.clone(), keys, callback))
    .chain(ast_values.into_iter().map(|v| parsed::Clause::S(PType::Par, Rc::new(v))));
  parsed::Clause::s('(', items, range)
}
