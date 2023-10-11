use std::any::{Any, TypeId};
use std::rc::Rc;

use hashbrown::HashMap;
use trait_set::trait_set;

use super::{run, Context, Return, RuntimeError};
use crate::foreign::{Atom, Atomic, ExternError};
use crate::interpreted::{Clause, Expr, ExprInst};
use crate::utils::take_with_output;

trait_set! {
  trait Handler = FnMut(Box<dyn Any>) -> HandlerRes;
}

/// A table of command handlers
#[derive(Default)]
pub struct HandlerTable<'a> {
  handlers: HashMap<TypeId, Box<dyn Handler + 'a>>,
}
impl<'a> HandlerTable<'a> {
  /// Create a new [HandlerTable]
  #[must_use]
  pub fn new() -> Self { Self { handlers: HashMap::new() } }

  /// Add a handler function to interpret a type of atom and decide what happens
  /// next. This function can be impure.
  pub fn register<T: 'static>(
    &mut self,
    mut f: impl FnMut(Box<T>) -> HandlerRes + 'a,
  ) {
    let cb = move |a: Box<dyn Any>| f(a.downcast().expect("found by TypeId"));
    let prev = self.handlers.insert(TypeId::of::<T>(), Box::new(cb));
    assert!(prev.is_none(), "A handler for this type is already registered");
  }

  /// Find and execute the corresponding handler for this type
  pub fn dispatch(
    &mut self,
    arg: Box<dyn Atomic>,
  ) -> Result<HandlerRes, Box<dyn Atomic>> {
    match self.handlers.get_mut(&arg.as_any_ref().type_id()) {
      Some(f) => Ok(f(arg.as_any())),
      None => Err(arg),
    }
  }

  /// Combine two non-overlapping handler sets
  #[must_use]
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
    let mut ret = run(expr, ctx.clone())?;
    let quit = take_with_output(&mut ret.state, |exi| match exi.expr_val() {
      Expr { clause: Clause::Atom(a), .. } => {
        match handlers.dispatch(a.0) {
          Err(b) => (Clause::Atom(Atom(b)).wrap(), Ok(true)),
          Ok(e) => match e {
            Ok(expr) => (expr, Ok(false)),
            Err(e) => (Clause::Bottom.wrap(), Err(e)),
          },
        }
      },
      expr => (ExprInst::new(expr), Ok(true)),
    })?;
    if quit | ret.gas.map_or(false, |g| g == 0) {
      return Ok(ret);
    }
    ctx.gas = ret.gas;
    expr = ret.state;
  }
}
