use std::fmt::Debug;

use crate::foreign::{Atomic, AtomicReturn};
use crate::interner::InternedDisplay;
use crate::interpreter::Context;
use crate::representations::interpreted::ExprInst;
use crate::{atomic_defaults, externfn_impl};

/// Print and return whatever expression is in the argument without normalizing
/// it.
///
/// Next state: [Debug1]
#[derive(Clone)]
pub struct Debug2;
externfn_impl!(Debug2, |_: &Self, x: ExprInst| Ok(Debug1 { x }));

/// Prev state: [Debug2]
#[derive(Debug, Clone)]
pub struct Debug1 {
  x: ExprInst,
}
impl Atomic for Debug1 {
  atomic_defaults!();
  fn run(&self, ctx: Context) -> crate::foreign::AtomicResult {
    println!("{}", self.x.bundle(ctx.interner));
    Ok(AtomicReturn {
      clause: self.x.expr().clause.clone(),
      gas: ctx.gas.map(|g| g - 1),
      inert: false,
    })
  }
}
