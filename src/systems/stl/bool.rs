use crate::foreign::{xfn_1ary, xfn_2ary, InertAtomic, XfnResult};
use crate::interner::Interner;
use crate::representations::interpreted::Clause;
use crate::systems::AssertionError;
use crate::{ConstTree, Literal, Location};

/// Booleans exposed to Orchid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Boolean(pub bool);
impl InertAtomic for Boolean {
  fn type_str() -> &'static str { "a boolean" }
}

impl From<bool> for Boolean {
  fn from(value: bool) -> Self { Self(value) }
}

/// Takes a boolean and two branches, runs the first if the bool is true, the
/// second if it's false.
// Even though it's a ternary function, IfThenElse is implemented as an unary
// foreign function, as the rest of the logic can be defined in Orchid.
pub fn if_then_else(b: Boolean) -> XfnResult<Clause> {
  Ok(match b.0 {
    true => Clause::pick(Clause::constfn(Clause::LambdaArg)),
    false => Clause::constfn(Clause::pick(Clause::LambdaArg)),
  })
}

/// Compares the inner values if
///
/// - both are string,
/// - both are either uint or num
pub fn equals(a: Literal, b: Literal) -> XfnResult<Boolean> {
  Ok(Boolean::from(match (a, b) {
    (Literal::Str(s1), Literal::Str(s2)) => s1 == s2,
    (Literal::Num(n1), Literal::Num(n2)) => n1 == n2,
    (Literal::Uint(i1), Literal::Uint(i2)) => i1 == i2,
    (Literal::Num(n1), Literal::Uint(u1)) => *n1 == (u1 as f64),
    (Literal::Uint(u1), Literal::Num(n1)) => *n1 == (u1 as f64),
    (..) => AssertionError::fail(Location::Unknown, "the expected type")?,
  }))
}

pub fn bool(i: &Interner) -> ConstTree {
  ConstTree::tree([(
    i.i("bool"),
    ConstTree::tree([
      (i.i("ifthenelse"), ConstTree::xfn(xfn_1ary(if_then_else))),
      (i.i("equals"), ConstTree::xfn(xfn_2ary(equals))),
      (i.i("true"), ConstTree::atom(Boolean(true))),
      (i.i("false"), ConstTree::atom(Boolean(false))),
    ]),
  )])
}
