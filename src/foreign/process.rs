use std::fmt::Debug;

use super::atom::{Atomic, AtomicReturn};
use super::error::ExternResult;
use super::to_clause::ToClause;
use crate::interpreter::apply::CallData;
use crate::interpreter::nort::{Clause, ClauseInst};
use crate::interpreter::run::RunData;
use crate::utils::ddispatch::Responder;

/// An atom that immediately decays to the result of the function when
/// normalized. Can be used to build infinite recursive datastructures from
/// Rust.
#[derive(Clone)]
pub struct Unstable<F>(F);
impl<F: FnOnce(RunData) -> R + Send + 'static, R: ToClause> Unstable<F> {
  /// Wrap a function in an Unstable
  pub const fn new(f: F) -> Self { Self(f) }
}
impl<F> Responder for Unstable<F> {}
impl<F> Debug for Unstable<F> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Unstable").finish_non_exhaustive()
  }
}
impl<F: FnOnce(RunData) -> R + Send + 'static, R: ToClause> Atomic
  for Unstable<F>
{
  fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> { self }
  fn as_any_ref(&self) -> &dyn std::any::Any { self }
  fn apply_ref(&self, _: CallData) -> ExternResult<Clause> {
    panic!("This atom decays instantly")
  }
  fn run(self: Box<Self>, run: RunData) -> super::atom::AtomicResult {
    let clause = self.0(run.clone()).to_clause(run.location.clone());
    AtomicReturn::run(clause, run)
  }
  fn redirect(&mut self) -> Option<&mut ClauseInst> { None }
}
