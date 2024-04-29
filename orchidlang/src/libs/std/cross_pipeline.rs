use std::iter;
use std::rc::Rc;

use itertools::Itertools;

use crate::foreign::atom::Atomic;
use crate::foreign::fn_bridge::xfn;
use crate::foreign::process::Unstable;
use crate::foreign::to_clause::ToClause;
use crate::foreign::try_from_expr::{TryFromExpr, WithLoc};
use crate::location::SourceRange;
use crate::parse::parsed::{self, PType};
use crate::utils::pure_seq::pushed;

pub trait DeferredRuntimeCallback<T, R: ToClause>:
  FnOnce(Vec<T>) -> R + Clone + Send + 'static
{
}
impl<T, R: ToClause, F: FnOnce(Vec<T>) -> R + Clone + Send + 'static> DeferredRuntimeCallback<T, R>
  for F
{
}

/// Lazy-recursive function that takes the next value from the interpreter
/// and acts upon it
///
/// # Panics
///
/// If the list of remaining keys is empty
fn table_receiver_rec<T: TryFromExpr + Clone + Send + 'static, R: ToClause + 'static>(
  results: Vec<T>,
  items: usize,
  callback: impl DeferredRuntimeCallback<T, R>,
) -> impl Atomic {
  xfn("__table_receiver__", move |WithLoc(loc, t): WithLoc<T>| {
    let results: Vec<T> = pushed(results, t);
    match items == results.len() {
      true => callback(results).to_clause(loc),
      false => table_receiver_rec(results, items, callback).atom_cls(),
    }
  })
}

fn table_receiver<T: TryFromExpr + Clone + Send + 'static, R: ToClause + 'static>(
  items: usize,
  callback: impl DeferredRuntimeCallback<T, R>,
) -> parsed::Clause {
  if items == 0 {
    Unstable::new(move |_| callback(Vec::new())).ast_cls()
  } else {
    Unstable::new(move |_| table_receiver_rec(Vec::new(), items, callback).atom_cls()).ast_cls()
  }
}

/// Defers the execution of the callback to runtime, allowing it to depend on
/// the result of Otchid expressions.
pub fn defer_to_runtime<T: TryFromExpr + Clone + Send + 'static, R: ToClause + 'static>(
  range: SourceRange,
  exprs: impl Iterator<Item = Vec<parsed::Expr>>,
  callback: impl DeferredRuntimeCallback<T, R>,
) -> parsed::Clause {
  let argv = exprs.into_iter().map(|v| parsed::Clause::S(PType::Par, Rc::new(v))).collect_vec();
  let items = iter::once(table_receiver(argv.len(), callback)).chain(argv);
  parsed::Clause::s('(', items, range)
}
