use std::fmt::Debug;

use super::io::IO;
use crate::foreign::{Atomic, AtomicResult, AtomicReturn};
use crate::interpreter::Context;
use crate::representations::interpreted::ExprInst;
use crate::{atomic_defaults, externfn_impl};

/// Create an [IO] event that reads a line form standard input and calls the
/// continuation with it.
///
/// Next state: [Readln1]
#[derive(Clone)]
pub struct Readln2;
externfn_impl!(Readln2, |_: &Self, x: ExprInst| Ok(Readln1 { x }));

/// Prev state: [Readln2]
#[derive(Debug, Clone)]
pub struct Readln1 {
  x: ExprInst,
}
impl Atomic for Readln1 {
  atomic_defaults!();
  fn run(&self, ctx: Context) -> AtomicResult {
    Ok(AtomicReturn::from_data(IO::Readline(self.x.clone()), ctx))
  }
}
