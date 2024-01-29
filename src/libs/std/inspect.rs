use std::fmt::Debug;

use crate::foreign::atom::{Atomic, AtomicResult, AtomicReturn, CallData, RunData};
use crate::foreign::error::ExternResult;
use crate::foreign::to_clause::ToClause;
use crate::gen::tree::{atom_ent, xfn_ent, ConstTree};
use crate::interpreter::nort::{Clause, Expr};
use crate::utils::ddispatch::Responder;

#[derive(Debug, Clone)]
struct Inspect;
impl Responder for Inspect {} 
impl Atomic for Inspect {
  fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> { self }
  fn as_any_ref(&self) -> &dyn std::any::Any { self }
  fn redirect(&mut self) -> Option<&mut Expr> { None }
  fn run(self: Box<Self>, _: RunData) -> AtomicResult { AtomicReturn::inert(*self) }
  fn apply_ref(&self, call: CallData) -> ExternResult<Clause> {
    eprintln!("{}", call.arg);
    Ok(call.arg.to_clause(call.location))
  }
} 

pub fn inspect_lib() -> ConstTree {
  ConstTree::ns("std", [ConstTree::tree([
    atom_ent("inspect", [Inspect]),
    xfn_ent("tee", [|x: Expr| {
      eprintln!("{x}");
      x
    }]),
  ])])
}
