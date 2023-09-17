//! Utilities for generating Orchid code in Rust

use std::rc::Rc;

use crate::interpreted::{Clause, ExprInst};
use crate::utils::unwrap_or;
use crate::{PathSet, Side};

/// Convert a rust Option into an Orchid Option
pub fn orchid_opt(x: Option<ExprInst>) -> Clause {
  if let Some(x) = x { some(x) } else { none() }
}

/// Constructs an instance of the orchid value Some wrapping the given
/// [ExprInst].
///
/// Takes two expressions and calls the second with the given data
fn some(x: ExprInst) -> Clause {
  Clause::Lambda {
    args: None,
    body: Clause::Lambda {
      args: Some(PathSet { steps: Rc::new(vec![Side::Left]), next: None }),
      body: Clause::Apply { f: Clause::LambdaArg.wrap(), x }.wrap(),
    }
    .wrap(),
  }
}

/// Constructs an instance of the orchid value None
///
/// Takes two expressions and returns the first
fn none() -> Clause {
  Clause::Lambda {
    args: Some(PathSet { steps: Rc::new(vec![]), next: None }),
    body: Clause::Lambda { args: None, body: Clause::LambdaArg.wrap() }.wrap(),
  }
}

/// Define a clause that can be called with a callback and passes the provided
/// values to the callback in order.
pub fn tuple(data: impl IntoIterator<Item = ExprInst>) -> Clause {
  let mut steps = Vec::new();
  let mut body = Clause::LambdaArg.wrap();
  for x in data.into_iter() {
    steps.push(Side::Left);
    body = Clause::Apply { f: body, x }.wrap()
  }
  let path_set = PathSet { next: None, steps: Rc::new(steps) };
  Clause::Lambda { args: Some(path_set), body }
}

#[cfg(test)]
mod test {
    use crate::systems::codegen::tuple;

  #[test]
  fn tuple_printer() {
    println!("Binary tuple: {}", tuple([0.into(), 1.into()]))
  }
}

/// Generate a function call with the specified arugment array.
pub fn call(f: ExprInst, args: impl IntoIterator<Item = ExprInst>) -> Clause {
  let mut it = args.into_iter();
  let x = unwrap_or!(it.by_ref().next(); return f.inspect(Clause::clone));
  it.fold(Clause::Apply { f, x }, |acc, x| Clause::Apply { f: acc.wrap(), x })
}

/// Build an Orchid list from a Rust iterator
pub fn list(items: impl IntoIterator<Item = ExprInst>) -> Clause {
  let mut iter = items.into_iter();
  orchid_opt(iter.next().map(|it| tuple([it, list(iter).wrap()]).wrap()))
}
