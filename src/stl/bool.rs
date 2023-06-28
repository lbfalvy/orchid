use std::rc::Rc;

use crate::foreign::Atom;
use crate::interner::Interner;
use crate::representations::interpreted::{Clause, ExprInst};
use crate::representations::Primitive;
use crate::stl::litconv::with_lit;
use crate::stl::AssertionError;
use crate::{atomic_inert, define_fn, ConstTree, Literal, PathSet};

/// Booleans exposed to Orchid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Boolean(pub bool);
atomic_inert!(Boolean);

impl From<bool> for Boolean {
  fn from(value: bool) -> Self {
    Self(value)
  }
}

impl TryFrom<&ExprInst> for Boolean {
  type Error = ();

  fn try_from(value: &ExprInst) -> Result<Self, Self::Error> {
    let expr = value.expr();
    if let Clause::P(Primitive::Atom(Atom(a))) = &expr.clause {
      if let Some(b) = a.as_any().downcast_ref::<Boolean>() {
        return Ok(*b);
      }
    }
    Err(())
  }
}

define_fn! {expr=x in
  /// Compares the inner values if
  ///
  /// - both values are char,
  /// - both are string,
  /// - both are either uint or num
  Equals {
    a: Literal as with_lit(x, |l| Ok(l.clone())),
    b: Literal as with_lit(x, |l| Ok(l.clone()))
  } => Ok(Boolean::from(match (a, b) {
    (Literal::Char(c1), Literal::Char(c2)) => c1 == c2,
    (Literal::Num(n1), Literal::Num(n2)) => n1 == n2,
    (Literal::Str(s1), Literal::Str(s2)) => s1 == s2,
    (Literal::Uint(i1), Literal::Uint(i2)) => i1 == i2,
    (Literal::Num(n1), Literal::Uint(u1)) => *n1 == (*u1 as f64),
    (Literal::Uint(u1), Literal::Num(n1)) => *n1 == (*u1 as f64),
    (..) => AssertionError::fail(
      b.clone().into(),
      "the expected type"
    )?,
  }).to_atom_cls())
}

// Even though it's a ternary function, IfThenElse is implemented as an unary
// foreign function, as the rest of the logic can be defined in Orchid.
define_fn! {
  /// Takes a boolean and two branches, runs the first if the bool is true, the
  /// second if it's false.
  IfThenElse = |x: &ExprInst| x.try_into()
    .map_err(|_| AssertionError::ext(x.clone(), "a boolean"))
    .map(|b: Boolean| if b.0 {Clause::Lambda {
      args: Some(PathSet { steps: Rc::new(vec![]), next: None }),
      body: Clause::Lambda {
        args: None,
        body: Clause::LambdaArg.wrap()
      }.wrap(),
    }} else {Clause::Lambda {
      args: None,
      body: Clause::Lambda {
        args: Some(PathSet { steps: Rc::new(vec![]), next: None }),
        body: Clause::LambdaArg.wrap(),
      }.wrap(),
    }})
}

pub fn bool(i: &Interner) -> ConstTree {
  ConstTree::tree([(
    i.i("bool"),
    ConstTree::tree([
      (i.i("ifthenelse"), ConstTree::xfn(IfThenElse)),
      (i.i("equals"), ConstTree::xfn(Equals)),
      (i.i("true"), ConstTree::atom(Boolean(true))),
      (i.i("false"), ConstTree::atom(Boolean(false))),
    ]),
  )])
}
