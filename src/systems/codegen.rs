//! Utilities for generating Orchid code in Rust

use crate::interpreted::{Clause, ExprInst};
use crate::utils::unwrap_or;
use crate::{PathSet, Side};

/// Convert a rust Option into an Orchid Option
pub fn opt(x: Option<ExprInst>) -> Clause {
  match x {
    Some(x) => Clause::constfn(Clause::lambda(
      PathSet::end([Side::Left]),
      Clause::Apply { f: Clause::LambdaArg.wrap(), x },
    )),
    None => Clause::pick(Clause::constfn(Clause::LambdaArg)),
  }
}

/// Convert a rust Result into an Orchid Result
pub fn res(x: Result<ExprInst, ExprInst>) -> Clause {
  let mk_body = |x| Clause::Apply { f: Clause::LambdaArg.wrap(), x };
  let pick_fn = |b| Clause::lambda(PathSet::end([Side::Left]), b);
  match x {
    Ok(x) => Clause::constfn(pick_fn(mk_body(x))),
    Err(x) => pick_fn(Clause::constfn(mk_body(x))),
  }
}

/// Define a clause that can be called with a callback and passes the provided
/// values to the callback in order.
pub fn tuple(data: impl IntoIterator<Item = ExprInst>) -> Clause {
  let mut steps = Vec::new();
  let mut body = Clause::LambdaArg;
  for x in data.into_iter() {
    steps.push(Side::Left);
    body = Clause::Apply { f: body.wrap(), x }
  }
  Clause::lambda(PathSet::end(steps), body)
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
  opt(iter.next().map(|it| tuple([it, list(iter).wrap()]).wrap()))
}
