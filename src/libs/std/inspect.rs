use std::fmt::Debug;

use crate::foreign::atom::{Atomic, AtomicResult, AtomicReturn};
use crate::foreign::error::ExternResult;
use crate::foreign::fn_bridge::constructors::xfn_1ary;
use crate::foreign::to_clause::ToClause;
use crate::gen::tree::{atom_leaf, ConstTree};
use crate::interpreter::apply::CallData;
use crate::interpreter::nort::{Clause, ClauseInst, Expr};
use crate::interpreter::run::RunData;
use crate::utils::ddispatch::Responder;

#[derive(Debug, Clone)]
struct Inspect;
impl Responder for Inspect {}
impl Atomic for Inspect {
  fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> { self }
  fn as_any_ref(&self) -> &dyn std::any::Any { self }
  fn redirect(&mut self) -> Option<&mut ClauseInst> { None }
  fn run(self: Box<Self>, run: RunData) -> AtomicResult {
    AtomicReturn::inert(*self, run.ctx)
  }
  fn apply_ref(&self, call: CallData) -> ExternResult<Clause> {
    eprintln!("{}", call.arg);
    Ok(call.arg.to_clause(call.location))
  }
}

fn tee(x: Expr) -> Expr {
  eprintln!("{x}");
  x
}

pub fn inspect_lib() -> ConstTree {
  ConstTree::ns("std", [ConstTree::tree([
    ("inspect", atom_leaf(Inspect)),
    ("tee", atom_leaf(xfn_1ary(tee))),
  ])])
}
