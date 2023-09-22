use std::fmt::Debug;

use crate::foreign::{ExternFn, XfnResult};
use crate::interpreted::Clause;
use crate::interpreter::Context;
use crate::representations::interpreted::ExprInst;
use crate::{ConstTree, Interner};

#[derive(Debug, Clone)]
struct Inspect;
impl ExternFn for Inspect {
  fn name(&self) -> &str { "inspect" }
  fn apply(self: Box<Self>, arg: ExprInst, _: Context) -> XfnResult<Clause> {
    println!("{arg}");
    Ok(arg.expr().clause.clone())
  }
}

pub fn inspect(i: &Interner) -> ConstTree {
  ConstTree::tree([(i.i("inspect"), ConstTree::xfn(Inspect))])
}
