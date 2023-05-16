use std::fmt::Debug;

use crate::foreign::{Atomic, AtomicReturn};
use crate::interner::InternedDisplay;
use crate::interpreter::Context;
use crate::{externfn_impl, atomic_defaults};
use crate::representations::interpreted::ExprInst;

/// Debug function
/// 
/// Next state: [Debug0]

#[derive(Clone)]
pub struct Debug2;
externfn_impl!(Debug2, |_: &Self, x: ExprInst| Ok(Debug1{x}));

/// Partially applied Print function
/// 
/// Prev state: [Debug1]

#[derive(Debug, Clone)]
pub struct Debug1{ x: ExprInst }
impl Atomic for Debug1 {
  atomic_defaults!();
  fn run(&self, ctx: Context) -> crate::foreign::AtomicResult {
    println!("{}", self.x.bundle(&ctx.interner));
    Ok(AtomicReturn{
      clause: self.x.expr().clause.clone(),
      gas: ctx.gas.map(|g| g - 1),
      inert: false
    })
  }
}