//! Automated wrappers to make working with CPS commands easier.

use std::fmt::Debug;

use trait_set::trait_set;

use super::atom::{Atomic, AtomicResult, AtomicReturn, NotAFunction};
use super::error::{ExternError, ExternResult};
use crate::interpreter::apply::CallData;
use crate::interpreter::nort::{Clause, ClauseInst, Expr};
use crate::interpreter::run::RunData;
use crate::location::CodeLocation;
use crate::utils::ddispatch::{Request, Responder};
use crate::utils::pure_seq::pushed_ref;

trait_set! {
  /// A "well behaved" type that can be used as payload in a CPS box
  pub trait CPSPayload = Clone + Debug + Send + 'static;
  /// A function to handle a CPS box with a specific payload
  pub trait CPSHandler<T: CPSPayload> = FnMut(&T, &Expr) -> ExternResult<Expr>;
}

/// An Orchid Atom value encapsulating a payload and continuation points
#[derive(Debug, Clone)]
pub struct CPSBox<T: CPSPayload> {
  /// Number of arguments not provided yet
  pub argc: usize,
  /// Details about the command
  pub payload: T,
  /// Possible continuations, in the order they were provided
  pub continuations: Vec<Expr>,
}
impl<T: CPSPayload> CPSBox<T> {
  /// Create a new command prepared to receive exacly N continuations
  #[must_use]
  pub fn new(argc: usize, payload: T) -> Self {
    debug_assert!(argc > 0, "Null-ary CPS functions are invalid");
    Self { argc, continuations: Vec::new(), payload }
  }
  /// Unpack the wrapped command and the continuation
  #[must_use]
  pub fn unpack1(&self) -> (&T, Expr) {
    match &self.continuations[..] {
      [cont] => (&self.payload, cont.clone()),
      _ => panic!("size mismatch"),
    }
  }
  /// Unpack the wrapped command and 2 continuations (usually an async and a
  /// sync)
  #[must_use]
  pub fn unpack2(&self) -> (&T, Expr, Expr) {
    match &self.continuations[..] {
      [c1, c2] => (&self.payload, c1.clone(), c2.clone()),
      _ => panic!("size mismatch"),
    }
  }
  /// Unpack the wrapped command and 3 continuations (usually an async success,
  /// an async fail and a sync)
  #[must_use]
  pub fn unpack3(&self) -> (&T, Expr, Expr, Expr) {
    match &self.continuations[..] {
      [c1, c2, c3] => (&self.payload, c1.clone(), c2.clone(), c3.clone()),
      _ => panic!("size mismatch"),
    }
  }

  fn assert_applicable(&self, err_loc: &CodeLocation) -> ExternResult<()> {
    match self.argc {
      0 => Err(NotAFunction(self.clone().atom_expr(err_loc.clone())).rc()),
      _ => Ok(()),
    }
  }
}
impl<T: CPSPayload> Responder for CPSBox<T> {
  fn respond(&self, _request: Request) {}
}
impl<T: CPSPayload> Atomic for CPSBox<T> {
  fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> { self }
  fn as_any_ref(&self) -> &dyn std::any::Any { self }
  fn parser_eq(&self, _: &dyn std::any::Any) -> bool { false }
  fn redirect(&mut self) -> Option<&mut ClauseInst> { None }
  fn run(self: Box<Self>, run: RunData) -> AtomicResult {
    AtomicReturn::inert(*self, run.ctx)
  }
  fn apply(mut self: Box<Self>, call: CallData) -> ExternResult<Clause> {
    self.assert_applicable(&call.location)?;
    self.argc -= 1;
    self.continuations.push(call.arg);
    Ok(self.atom_cls())
  }
  fn apply_ref(&self, call: CallData) -> ExternResult<Clause> {
    self.assert_applicable(&call.location)?;
    let new = Self {
      argc: self.argc - 1,
      continuations: pushed_ref(&self.continuations, call.arg),
      payload: self.payload.clone(),
    };
    Ok(new.atom_cls())
  }
}
