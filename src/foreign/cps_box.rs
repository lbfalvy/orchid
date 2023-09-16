//! Automated wrappers to make working with CPS commands easier.

use std::fmt::Debug;

use trait_set::trait_set;

use super::{Atomic, ExternFn, InertAtomic, XfnResult};
use crate::interpreted::{Clause, ExprInst};
use crate::interpreter::{Context, HandlerRes};
use crate::utils::pure_push::pushed_ref;
use crate::ConstTree;

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
  fn name(&self) -> &str { "CPS function without argument" }
  fn apply(self: Box<Self>, arg: ExprInst, _ctx: Context) -> XfnResult {
    let payload = self.payload.clone();
    let continuations = pushed_ref(&self.continuations, arg);
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
  pub fn unpack1(self) -> (T, ExprInst) {
    self.assert_count(1);
    let [cont]: [ExprInst; 1] =
      self.continuations.try_into().expect("size checked");
    (self.payload, cont)
  }
  /// Unpack the wrapped command and 2 continuations (usually an async and a
  /// sync)
  pub fn unpack2(self) -> (T, ExprInst, ExprInst) {
    self.assert_count(2);
    let [c1, c2]: [ExprInst; 2] =
      self.continuations.try_into().expect("size checked");
    (self.payload, c1, c2)
  }
  /// Unpack the wrapped command and 3 continuations (usually an async success,
  /// an async fail and a sync)
  pub fn unpack3(self) -> (T, ExprInst, ExprInst, ExprInst) {
    self.assert_count(3);
    let [c1, c2, c3]: [ExprInst; 3] =
      self.continuations.try_into().expect("size checked");
    (self.payload, c1, c2, c3)
  }
}

impl<T: CPSPayload> InertAtomic for CPSBox<T> {
  fn type_str() -> &'static str { "a CPS box" }
}

/// Like [init_cps] but wrapped in a [ConstTree] for init-time usage
pub fn const_cps<T: CPSPayload>(argc: usize, payload: T) -> ConstTree {
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
