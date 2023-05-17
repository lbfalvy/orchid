use std::fmt::Debug;

use crate::foreign::{Atomic, AtomicResult, AtomicReturn};
use crate::interpreter::Context;
use crate::{externfn_impl, atomic_defaults};
use crate::representations::interpreted::ExprInst;

use super::io::IO;

/// Readln function
/// 
/// Next state: [Readln1]

#[derive(Clone)]
pub struct Readln2;
externfn_impl!(Readln2, |_: &Self, x: ExprInst| Ok(Readln1{x}));

/// Partially applied Readln function
/// 
/// Prev state: [Readln2]; Next state: [Readln0]

#[derive(Debug, Clone)]
pub struct Readln1{ x: ExprInst }
impl Atomic for Readln1 {
  atomic_defaults!();
  fn run(&self, ctx: Context) -> AtomicResult {
    Ok(AtomicReturn::from_data(
      IO::Readline(self.x.clone()), 
      ctx
    ))
  }
}