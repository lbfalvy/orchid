use std::fmt::Debug;

use super::super::litconv::with_str;
use super::io::IO;
use crate::foreign::{Atomic, AtomicResult, AtomicReturn};
use crate::interpreter::Context;
use crate::representations::interpreted::ExprInst;
use crate::{atomic_defaults, atomic_impl, atomic_redirect, externfn_impl};

/// Wrap a string and the continuation into an [IO] event to be evaluated by the
/// embedder.
///
/// Next state: [Print1]
#[derive(Clone)]
pub struct Print2;
externfn_impl!(Print2, |_: &Self, x: ExprInst| Ok(Print1 { x }));

/// Prev state: [Print2]; Next state: [Print0]
#[derive(Debug, Clone)]
pub struct Print1 {
  x: ExprInst,
}
atomic_redirect!(Print1, x);
atomic_impl!(Print1);
externfn_impl!(Print1, |this: &Self, x: ExprInst| {
  with_str(&this.x, |s| Ok(Print0 { s: s.clone(), x }))
});

/// Prev state: [Print1]
#[derive(Debug, Clone)]
pub struct Print0 {
  s: String,
  x: ExprInst,
}
impl Atomic for Print0 {
  atomic_defaults!();
  fn run(&self, ctx: Context) -> AtomicResult {
    Ok(AtomicReturn::from_data(IO::Print(self.s.clone(), self.x.clone()), ctx))
  }
}