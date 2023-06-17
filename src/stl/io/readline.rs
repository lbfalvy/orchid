use std::fmt::Debug;

use super::command::IO;
use crate::foreign::{Atomic, AtomicResult, AtomicReturn};
use crate::interpreter::Context;
use crate::representations::interpreted::ExprInst;
use crate::{atomic_defaults, write_fn_step};

write_fn_step! {
  /// Create an [IO] event that reads a line form standard input and calls the
  /// continuation with it.
  pub ReadLn > ReadLn1
}

#[derive(Debug, Clone)]
struct ReadLn1 {
  expr_inst: ExprInst,
}
impl Atomic for ReadLn1 {
  atomic_defaults!();
  fn run(&self, ctx: Context) -> AtomicResult {
    let command = IO::Readline(self.expr_inst.clone());
    Ok(AtomicReturn::from_data(command, ctx))
  }
}
