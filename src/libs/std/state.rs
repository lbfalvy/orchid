use std::sync::{Arc, Mutex};

use crate::foreign::fn_bridge::Thunk;
use crate::foreign::inert::{Inert, InertPayload};
use crate::gen::tpl;
use crate::gen::traits::Gen;
use crate::gen::tree::{xfn_ent, ConstTree};
use crate::interpreter::gen_nort::nort_gen;
use crate::interpreter::handler::HandlerTable;
use crate::interpreter::nort::Expr;

#[derive(Debug, Clone)]
pub struct State(Arc<Mutex<Expr>>);
impl InertPayload for State {
  const TYPE_STR: &'static str = "State";
}

#[derive(Debug, Clone)]
struct NewStateCmd(Expr, Expr);
impl InertPayload for NewStateCmd {
  const TYPE_STR: &'static str = "NewStateCmd";
  fn strict_eq(&self, _: &Self) -> bool { true }
}
#[derive(Debug, Clone)]
struct SetStateCmd(State, Expr, Expr);
impl InertPayload for SetStateCmd {
  const TYPE_STR: &'static str = "SetStateCmd";
}

#[derive(Debug, Clone)]
struct GetStateCmd(State, Expr);
impl InertPayload for GetStateCmd {
  const TYPE_STR: &'static str = "GetStateCmd";
}

fn new_state(default: Thunk, cont: Thunk) -> Inert<NewStateCmd> {
  Inert(NewStateCmd(default.0, cont.0))
}

fn get_state(s: Inert<State>, cont: Thunk) -> Inert<GetStateCmd> {
  Inert(GetStateCmd(s.0, cont.0))
}

fn set_state(s: Inert<State>, value: Thunk, cont: Thunk) -> Inert<SetStateCmd> {
  Inert(SetStateCmd(s.0, value.0, cont.0))
}

fn new_state_handler(cmd: &Inert<NewStateCmd>) -> Expr {
  let Inert(NewStateCmd(default, handler)) = cmd;
  let state = State(Arc::new(Mutex::new(default.clone())));
  let tpl = tpl::A(tpl::Slot, tpl::V(Inert(state)));
  tpl.template(nort_gen(handler.location()), [handler.clone()])
}

fn set_state_handler(cmd: &Inert<SetStateCmd>) -> Expr {
  let Inert(SetStateCmd(state, value, handler)) = cmd;
  *state.0.lock().unwrap() = value.clone();
  handler.clone()
}

fn get_state_handler(cmd: &Inert<GetStateCmd>) -> Expr {
  let Inert(GetStateCmd(state, handler)) = cmd;
  let val = state.0.lock().unwrap().clone();
  let tpl = tpl::A(tpl::Slot, tpl::Slot);
  tpl.template(nort_gen(handler.location()), [handler.clone(), val])
}

pub fn state_handlers() -> HandlerTable<'static> {
  let mut handlers = HandlerTable::new();
  handlers.register(new_state_handler);
  handlers.register(get_state_handler);
  handlers.register(set_state_handler);
  handlers
}

pub fn state_lib() -> ConstTree {
  ConstTree::ns("std::state", [ConstTree::tree([
    xfn_ent("new_state", [new_state]),
    xfn_ent("get_state", [get_state]),
    xfn_ent("set_state", [set_state]),
  ])])
}
