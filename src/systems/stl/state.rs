use std::cell::RefCell;
use std::rc::Rc;

use crate::foreign::cps_box::{const_cps, init_cps, CPSBox};
use crate::foreign::{Atomic, InertAtomic};
use crate::interpreted::ExprInst;
use crate::interpreter::HandlerTable;
use crate::systems::codegen::call;
use crate::{define_fn, ConstTree, Interner};

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

define_fn! { SetState = |x| Ok(init_cps(2, SetStateCmd(x.downcast()?))) }
define_fn! { GetState = |x| Ok(init_cps(2, GetStateCmd(x.downcast()?))) }

fn new_state_handler<E>(cmd: &CPSBox<NewStateCmd>) -> Result<ExprInst, E> {
  let (_, default, handler) = cmd.unpack2();
  let state = State(Rc::new(RefCell::new(default.clone())));
  Ok(call(handler.clone(), [state.atom_exi()]).wrap())
}

fn set_state_handler<E>(cmd: &CPSBox<SetStateCmd>) -> Result<ExprInst, E> {
  let (SetStateCmd(state), value, handler) = cmd.unpack2();
  *state.0.as_ref().borrow_mut() = value.clone();
  Ok(handler.clone())
}

fn get_state_handler<E>(cmd: &CPSBox<GetStateCmd>) -> Result<ExprInst, E> {
  let (GetStateCmd(state), handler) = cmd.unpack1();
  Ok(call(handler.clone(), [state.0.as_ref().borrow().clone()]).wrap())
}

pub fn state_handlers() -> HandlerTable<'static> {
  let mut handlers = HandlerTable::new();
  handlers.register(new_state_handler);
  handlers.register(get_state_handler);
  handlers.register(set_state_handler);
  handlers
}

pub fn state_lib(i: &Interner) -> ConstTree {
  ConstTree::namespace(
    [i.i("state")],
    ConstTree::tree([
      (i.i("new_state"), const_cps(2, NewStateCmd)),
      (i.i("get_state"), ConstTree::xfn(GetState)),
      (i.i("set_state"), ConstTree::xfn(SetState)),
    ]),
  )
}
