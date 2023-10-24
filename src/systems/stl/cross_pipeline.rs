use std::collections::VecDeque;
use std::fmt::Debug;
use std::iter;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::ast::{self, PType};
use crate::ddispatch::Responder;
use crate::foreign::{
  xfn_1ary, Atomic, AtomicReturn, ExFn, StrictEq, ToClause, XfnResult,
};
use crate::interpreted::{self, TryFromExprInst};
use crate::utils::pure_seq::pushed;
use crate::{interpreter, VName};

pub trait DeferredRuntimeCallback<T, U, R: ToClause>:
  Fn(Vec<(T, U)>) -> XfnResult<R> + Clone + Send + 'static
{
}
impl<
  T,
  U,
  R: ToClause,
  F: Fn(Vec<(T, U)>) -> XfnResult<R> + Clone + Send + 'static,
> DeferredRuntimeCallback<T, U, R> for F
{
}

fn table_receiver_rec<
  T: Clone + Send + 'static,
  U: TryFromExprInst + Clone + Send + 'static,
  R: ToClause + 'static,
>(
  results: Vec<(T, U)>,
  mut remaining_keys: VecDeque<T>,
  callback: impl DeferredRuntimeCallback<T, U, R>,
) -> XfnResult<interpreted::Clause> {
  match remaining_keys.pop_front() {
    None => callback(results).map(|v| v.to_clause()),
    Some(t) => Ok(interpreted::Clause::ExternFn(ExFn(Box::new(xfn_1ary(
      move |u: U| {
        table_receiver_rec(pushed(results, (t, u)), remaining_keys, callback)
      },
    ))))),
  }
}

#[derive(Clone)]
pub struct EphemeralAtom(
  Arc<dyn Fn() -> XfnResult<interpreted::Clause> + Sync + Send>,
);
impl Debug for EphemeralAtom {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("EphemeralAtom")
  }
}
impl Responder for EphemeralAtom {
  fn respond(&self, _request: crate::ddispatch::Request) {}
}
impl StrictEq for EphemeralAtom {
  fn strict_eq(&self, _: &dyn std::any::Any) -> bool { false }
}
impl Atomic for EphemeralAtom {
  fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> { self }
  fn as_any_ref(&self) -> &dyn std::any::Any { self }
  fn run(
    self: Box<Self>,
    ctx: interpreter::Context,
  ) -> crate::foreign::AtomicResult {
    Ok(AtomicReturn { clause: (self.0)()?, gas: ctx.gas, inert: false })
  }
}

fn table_receiver<
  T: Clone + Send + 'static,
  U: TryFromExprInst + Clone + Send + 'static,
  R: ToClause + 'static,
>(
  keys: VecDeque<T>,
  callback: impl DeferredRuntimeCallback<T, U, R>,
) -> ast::Clause<VName> {
  if keys.is_empty() {
    let result =
      Arc::new(Mutex::new(callback(Vec::new()).map(|v| v.to_clause())));
    EphemeralAtom(Arc::new(move || result.lock().unwrap().deref().clone()))
      .ast_cls()
  } else {
    match table_receiver_rec(Vec::new(), keys, callback) {
      Ok(interpreted::Clause::ExternFn(xfn)) => ast::Clause::ExternFn(xfn),
      _ => unreachable!("details"),
    }
  }
}

pub fn defer_to_runtime<
  T: Clone + Send + 'static,
  U: TryFromExprInst + Clone + Send + 'static,
  R: ToClause + 'static,
>(
  pairs: impl IntoIterator<Item = (T, Vec<ast::Expr<VName>>)>,
  callback: impl DeferredRuntimeCallback<T, U, R>,
) -> ast::Clause<VName> {
  let (keys, ast_values) =
    pairs.into_iter().unzip::<_, _, VecDeque<_>, Vec<_>>();
  ast::Clause::s(
    '(',
    iter::once(table_receiver(keys, callback)).chain(
      ast_values.into_iter().map(|v| ast::Clause::S(PType::Par, Rc::new(v))),
    ),
  )
}
