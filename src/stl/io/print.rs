use std::fmt::Debug;

use super::super::inspect::with_str;
use super::command::IO;
use crate::foreign::{Atomic, AtomicResult, AtomicReturn};
use crate::interpreter::Context;
use crate::representations::interpreted::ExprInst;
use crate::{atomic_defaults, write_fn_step};

write_fn_step! {
  /// Wrap a string and the continuation into an [IO] event to be evaluated by
  /// the embedder.
  pub Print > Print1
}
write_fn_step! {
  Print1 {}
  Print0 where message = x => with_str(x, |s| Ok(s.clone()));
}

#[derive(Debug, Clone)]
struct Print0 {
  message: String,
  expr_inst: ExprInst,
}
impl Atomic for Print0 {
  atomic_defaults!();
  fn run(&self, ctx: Context) -> AtomicResult {
    let command = IO::Print(self.message.clone(), self.expr_inst.clone());
    Ok(AtomicReturn::from_data(command, ctx))
  }
}
