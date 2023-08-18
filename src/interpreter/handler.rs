use std::any::{Any, TypeId};
use std::rc::Rc;

use hashbrown::HashMap;
use trait_set::trait_set;

use super::{run, Context, Return, RuntimeError};
use crate::foreign::ExternError;
use crate::interpreted::{Clause, ExprInst};
use crate::Primitive;

trait_set! {
  trait Handler = for<'b> FnMut(&'b dyn Any) -> HandlerRes;
}

/// A table of command handlers
#[derive(Default)]
pub struct HandlerTable<'a> {
  handlers: HashMap<TypeId, Box<dyn Handler + 'a>>,
}
impl<'a> HandlerTable<'a> {
  /// Create a new [HandlerTable]
  pub fn new() -> Self {
    Self { handlers: HashMap::new() }
  }

  /// Add a handler function to interpret a type of atom and decide what happens
  /// next. This function can be impure.
  pub fn register<T: 'static>(
    &mut self,
    mut f: impl for<'b> FnMut(&'b T) -> HandlerRes + 'a,
  ) {
    let cb = move |a: &dyn Any| f(a.downcast_ref().expect("found by TypeId"));
    let prev = self.handlers.insert(TypeId::of::<T>(), Box::new(cb));
    assert!(prev.is_none(), "A handler for this type is already registered");
  }

  /// Find and execute the corresponding handler for this type
  pub fn dispatch(&mut self, arg: &dyn Any) -> Option<HandlerRes> {
    self.handlers.get_mut(&arg.type_id()).map(|f| f(arg))
  }

  /// Combine two non-overlapping handler sets
  pub fn combine(mut self, other: Self) -> Self {
    for (key, value) in other.handlers {
      let prev = self.handlers.insert(key, value);
      assert!(prev.is_none(), "Duplicate handlers")
    }
    self
  }
}

/// Various possible outcomes of a [Handler] execution. Ok returns control to
/// the interpreter. The meaning of Err is decided by the value in it.
pub type HandlerRes = Result<ExprInst, Rc<dyn ExternError>>;

/// [run] orchid code, executing any commands it returns using the specified
/// [Handler]s.
pub fn run_handler(
  mut expr: ExprInst,
  handlers: &mut HandlerTable,
  mut ctx: Context,
) -> Result<Return, RuntimeError> {
  loop {
    let ret = run(expr.clone(), ctx.clone())?;
    if let Clause::P(Primitive::Atom(a)) = &ret.state.expr().clause {
      if let Some(e) = handlers.dispatch(a.0.as_any()) {
        expr = e?;
        ctx.gas = ret.gas;
        if ret.gas.map_or(true, |g| g > 0) {
          continue;
        }
      }
    }
    return Ok(ret);
  }
}
