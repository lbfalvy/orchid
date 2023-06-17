use std::fmt::Debug;

use crate::foreign::{Atomic, AtomicReturn};
use crate::interner::InternedDisplay;
use crate::interpreter::Context;
use crate::representations::interpreted::ExprInst;
use crate::{atomic_defaults, write_fn_step};

write_fn_step! {
  /// Print and return whatever expression is in the argument without
  /// normalizing it.
  pub Inspect > Inspect1
}

#[derive(Debug, Clone)]
struct Inspect1 {
  expr_inst: ExprInst,
}
impl Atomic for Inspect1 {
  atomic_defaults!();
  fn run(&self, ctx: Context) -> crate::foreign::AtomicResult {
    println!("{}", self.expr_inst.bundle(ctx.interner));
    Ok(AtomicReturn {
      clause: self.expr_inst.expr().clause.clone(),
      gas: ctx.gas.map(|g| g - 1),
      inert: false,
    })
  }
}
