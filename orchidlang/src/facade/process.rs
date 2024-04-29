//! Run Orchid commands in the context of the loaded environment. Either
//! returned by [super::loader::Loader::proc], or constructed manually from the
//! return value of [super::merge_trees::merge_trees] and
//! [super::loader::Loader::handlers].

use hashbrown::HashMap;

use super::merge_trees::NortConst;
use crate::interpreter::context::{Halt, RunEnv, RunParams};
use crate::interpreter::error::RunError;
use crate::interpreter::handler::HandlerTable;
use crate::interpreter::nort::Expr;
use crate::interpreter::run::run;
use crate::name::Sym;

/// This struct ties the state of systems to loaded code, and allows to call
/// Orchid-defined functions
pub struct Process<'a>(RunEnv<'a>);
impl<'a> Process<'a> {
  /// Build a process from the return value of [crate::facade::merge_trees] and
  pub fn new(
    consts: impl IntoIterator<Item = (Sym, NortConst)>,
    handlers: HandlerTable<'a>,
  ) -> Self {
    let symbols: HashMap<_, _> = consts.into_iter().map(|(k, v)| (k, v.value)).collect();
    Self(RunEnv::new(handlers, move |sym, location| {
      symbols.get(&sym).cloned().ok_or_else(|| RunEnv::sym_not_found(sym, location))
    }))
  }

  /// Execute the given command in this process. If gas is specified, at most as
  /// many steps will be executed and then the partial result returned.
  ///
  /// This is useful to catch infinite loops or ensure that a tenant program
  /// yields
  pub fn run(&self, prompt: Expr, gas: Option<usize>) -> Result<Halt, RunError<'_>> {
    run(prompt, &self.0, &mut RunParams { stack: 1000, gas })
  }
}
