//! An [Atomic] implementor that runs a callback and turns into the return
//! value. Useful to generate expressions that depend on values in the
//! interpreter's context, and to defer the generation of expensive
//! subexpressions
use std::fmt;

use super::atom::{Atomic, AtomicResult, AtomicReturn, CallData, RunData};
use super::error::RTResult;
use super::to_clause::ToClause;
use crate::interpreter::nort::{Clause, Expr};
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
impl<F> fmt::Debug for Unstable<F> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Unstable").finish_non_exhaustive()
  }
}
impl<F: FnOnce(RunData) -> R + Send + 'static, R: ToClause> Atomic for Unstable<F> {
  fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> { self }
  fn as_any_ref(&self) -> &dyn std::any::Any { self }
  fn type_name(&self) -> &'static str { std::any::type_name::<Self>() }

  fn apply_mut(&mut self, _: CallData) -> RTResult<Clause> { panic!("This atom decays instantly") }
  fn run(self: Box<Self>, run: RunData) -> AtomicResult {
    let loc = run.location.clone();
    let clause = self.0(run).to_clause(loc);
    Ok(AtomicReturn::Change(0, clause))
  }
  fn redirect(&mut self) -> Option<&mut Expr> { None }
}
