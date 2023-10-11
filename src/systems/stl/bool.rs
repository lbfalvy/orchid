use crate::foreign::{xfn_1ary, xfn_2ary, XfnResult, Atom};
use crate::interner::Interner;
use crate::representations::interpreted::Clause;
use crate::error::AssertionError;
use crate::{ConstTree, Location, OrcString};

use super::Numeric;

/// Takes a boolean and two branches, runs the first if the bool is true, the
/// second if it's false.
// Even though it's a ternary function, IfThenElse is implemented as an unary
// foreign function, as the rest of the logic can be defined in Orchid.
pub fn if_then_else(b: bool) -> XfnResult<Clause> {
  Ok(match b {
    true => Clause::pick(Clause::constfn(Clause::LambdaArg)),
    false => Clause::constfn(Clause::pick(Clause::LambdaArg)),
  })
}

/// Compares the inner values if
///
/// - both are string,
/// - both are bool,
/// - both are either uint or num
pub fn equals(a: Atom, b: Atom) -> XfnResult<bool> {
  let (a, b) = match (a.try_downcast::<OrcString>(), b.try_downcast::<OrcString>()) {
    (Ok(a), Ok(b)) => return Ok(a == b),
    (Err(a), Err(b)) => (a, b),
    _ => return Ok(false),
  };
  match (a.request::<Numeric>(), b.request::<Numeric>()) {
    (Some(a), Some(b)) => return Ok(a.as_float() == b.as_float()),
    (None, None) => (),
    _ => return Ok(false),
  };
  match (a.try_downcast::<bool>(), b.try_downcast::<bool>()) {
    (Ok(a), Ok(b)) => return Ok(a == b),
    (Err(_), Err(_)) => (),
    _ => return Ok(false),
  };
  AssertionError::fail(Location::Unknown, "the expected type")
}

pub fn bool(i: &Interner) -> ConstTree {
  ConstTree::tree([(
    i.i("bool"),
    ConstTree::tree([
      (i.i("ifthenelse"), ConstTree::xfn(xfn_1ary(if_then_else))),
      (i.i("equals"), ConstTree::xfn(xfn_2ary(equals))),
      (i.i("true"), ConstTree::atom(true)),
      (i.i("false"), ConstTree::atom(false)),
    ]),
  )])
}
