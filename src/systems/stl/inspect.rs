use std::fmt::Debug;

use crate::foreign::{Atomic, AtomicReturn};
use crate::interpreter::Context;
use crate::representations::interpreted::ExprInst;
use crate::utils::ddispatch::Responder;
use crate::{write_fn_step, ConstTree, Interner};

write_fn_step! {
  /// Print and return whatever expression is in the argument without
  /// normalizing it.
  Inspect > Inspect1
}

#[derive(Debug, Clone)]
struct Inspect1 {
  expr_inst: ExprInst,
}
impl Responder for Inspect1 {}
impl Atomic for Inspect1 {
  fn as_any(&self) -> &dyn std::any::Any { self }
  fn run(&self, ctx: Context) -> crate::foreign::AtomicResult {
    println!("{}", self.expr_inst);
    Ok(AtomicReturn {
      clause: self.expr_inst.expr().clause.clone(),
      gas: ctx.gas.map(|g| g - 1),
      inert: false,
    })
  }
}

pub fn inspect(i: &Interner) -> ConstTree {
  ConstTree::tree([(i.i("inspect"), ConstTree::xfn(Inspect))])
}
