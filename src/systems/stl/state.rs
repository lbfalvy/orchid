use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

use crate::foreign::cps_box::{const_cps, init_cps, CPSBox};
use crate::foreign::{xfn_1ary, Atomic, InertAtomic, XfnResult};
use crate::interpreted::{Clause, ExprInst};
use crate::interpreter::HandlerTable;
use crate::systems::codegen::call;
use crate::{ConstTree, Interner};

#[derive(Debug, Clone)]
pub struct State(Rc<RefCell<ExprInst>>);
impl InertAtomic for State {
  fn type_str() -> &'static str { "a stateful container" }
}

#[derive(Debug, Clone)]
struct NewStateCmd;

#[derive(Debug, Clone)]
struct SetStateCmd(State);

#[derive(Debug, Clone)]
struct GetStateCmd(State);

fn get_state(s: State) -> XfnResult<Clause> { Ok(init_cps(2, GetStateCmd(s))) }

fn set_state(s: State) -> XfnResult<Clause> { Ok(init_cps(2, SetStateCmd(s))) }

fn new_state_handler<E>(cmd: CPSBox<NewStateCmd>) -> Result<ExprInst, E> {
  let (_, default, handler) = cmd.unpack2();
  let state = State(Rc::new(RefCell::new(default)));
  Ok(call(handler, [state.atom_exi()]).wrap())
}

fn set_state_handler<E>(cmd: CPSBox<SetStateCmd>) -> Result<ExprInst, E> {
  let (SetStateCmd(state), value, handler) = cmd.unpack2();
  *state.0.as_ref().borrow_mut() = value;
  Ok(handler)
}

fn get_state_handler<E>(cmd: CPSBox<GetStateCmd>) -> Result<ExprInst, E> {
  let (GetStateCmd(state), handler) = cmd.unpack1();
  let val = match Rc::try_unwrap(state.0) {
    Ok(cell) => cell.into_inner(),
    Err(rc) => rc.as_ref().borrow().deref().clone(),
  };
  Ok(call(handler, [val]).wrap())
}

pub fn state_handlers() -> HandlerTable<'static> {
  let mut handlers = HandlerTable::new();
  handlers.register(|b| new_state_handler(*b));
  handlers.register(|b| get_state_handler(*b));
  handlers.register(|b| set_state_handler(*b));
  handlers
}

pub fn state_lib(i: &Interner) -> ConstTree {
  ConstTree::namespace(
    [i.i("state")],
    ConstTree::tree([
      (i.i("new_state"), const_cps(2, NewStateCmd)),
      (i.i("get_state"), ConstTree::xfn(xfn_1ary(get_state))),
      (i.i("set_state"), ConstTree::xfn(xfn_1ary(set_state))),
    ]),
  )
}
