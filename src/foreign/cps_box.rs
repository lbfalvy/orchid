//! Automated wrappers to make working with CPS commands easier.

use std::fmt::Debug;
use std::iter;

use trait_set::trait_set;

use super::{Atomic, AtomicResult, AtomicReturn, ExternFn, XfnResult};
use crate::interpreted::{Clause, ExprInst};
use crate::interpreter::{Context, HandlerRes};
use crate::{atomic_defaults, ConstTree};

trait_set! {
  /// A "well behaved" type that can be used as payload in a CPS box
  pub trait CPSPayload = Clone + Debug + 'static;
  /// A function to handle a CPS box with a specific payload
  pub trait CPSHandler<T: CPSPayload> = FnMut(&T, &ExprInst) -> HandlerRes;
}

/// The pre-argument version of CPSBox
#[derive(Debug, Clone)]
struct CPSFn<T: CPSPayload> {
  pub argc: usize,
  pub continuations: Vec<ExprInst>,
  pub payload: T,
}
impl<T: CPSPayload> CPSFn<T> {
  fn new(argc: usize, payload: T) -> Self {
    debug_assert!(
      argc > 0,
      "Null-ary CPS functions are invalid, use an Atom instead"
    );
    Self { argc, continuations: Vec::new(), payload }
  }
}
impl<T: CPSPayload> ExternFn for CPSFn<T> {
  fn name(&self) -> &str {
    "CPS function without argument"
  }
  fn apply(&self, arg: ExprInst, _ctx: Context) -> XfnResult {
    let payload = self.payload.clone();
    let continuations = (self.continuations.iter())
      .cloned()
      .chain(iter::once(arg))
      .collect::<Vec<_>>();
    if self.argc == 1 {
      Ok(CPSBox { payload, continuations }.atom_cls())
    } else {
      Ok(CPSFn { argc: self.argc - 1, payload, continuations }.xfn_cls())
    }
  }
}

/// An inert Orchid Atom value encapsulating a payload and a continuation point
#[derive(Debug, Clone)]
pub struct CPSBox<T: CPSPayload> {
  /// Details about the command
  pub payload: T,
  /// Possible continuations, in the order they were provided
  pub continuations: Vec<ExprInst>,
}
impl<T: CPSPayload> CPSBox<T> {
  /// Assert that the command was instantiated with the correct number of
  /// possible continuations. This is decided by the native bindings, not user
  /// code, therefore this error may be uncovered by usercode but can never be
  /// produced at will.
  pub fn assert_count(&self, expect: usize) {
    let real = self.continuations.len();
    debug_assert!(
      real == expect,
      "Tried to read {expect} argument(s) but {real} were provided for {:?}",
      self.payload
    )
  }
  /// Unpack the wrapped command and the continuation
  pub fn unpack1(&self) -> (&T, &ExprInst) {
    self.assert_count(1);
    (&self.payload, &self.continuations[0])
  }
  /// Unpack the wrapped command and 2 continuations (usually an async and a
  /// sync)
  pub fn unpack2(&self) -> (&T, &ExprInst, &ExprInst) {
    self.assert_count(2);
    (&self.payload, &self.continuations[0], &self.continuations[1])
  }
  /// Unpack the wrapped command and 3 continuations (usually an async success,
  /// an async fail and a sync)
  pub fn unpack3(&self) -> (&T, &ExprInst, &ExprInst, &ExprInst) {
    self.assert_count(3);
    (
      &self.payload,
      &self.continuations[0],
      &self.continuations[1],
      &self.continuations[2],
    )
  }
}

impl<T: CPSPayload> Atomic for CPSBox<T> {
  atomic_defaults!();
  fn run(&self, ctx: Context) -> AtomicResult {
    Ok(AtomicReturn {
      clause: self.clone().atom_cls(),
      gas: ctx.gas,
      inert: true,
    })
  }
}

/// Like [init_cps] but wrapped in a [ConstTree] for init-time usage
pub fn mk_const<T: CPSPayload>(argc: usize, payload: T) -> ConstTree {
  ConstTree::xfn(CPSFn::new(argc, payload))
}

/// Construct a CPS function which takes an argument and then acts inert
/// so that command executors can receive it.
///
/// This function is meant to be used in an external function defined with
/// [crate::define_fn]. For usage in a [ConstTree], see [mk_const]
pub fn init_cps<T: CPSPayload>(argc: usize, payload: T) -> Clause {
  CPSFn::new(argc, payload).xfn_cls()
}
