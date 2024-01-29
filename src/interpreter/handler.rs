use std::any::{Any, TypeId};

use hashbrown::HashMap;
use trait_set::trait_set;

use super::context::{Halt, RunContext};
use super::error::RunError;
use super::nort::{Clause, Expr};
use super::run::run;
use crate::foreign::atom::{Atom, Atomic};
use crate::foreign::error::ExternResult;
use crate::foreign::to_clause::ToClause;
use crate::location::CodeLocation;

trait_set! {
  trait Handler = for<'a> FnMut(&'a dyn Any, CodeLocation) -> Expr;
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

  /// Add a handler function to interpret a command and select the continuation.
  /// See [HandlerTable#with] for a declarative option.
  pub fn register<T: 'static, R: ToClause>(
    &mut self,
    mut f: impl for<'b> FnMut(&'b T) -> R + 'a,
  ) {
    let cb = move |a: &dyn Any, loc: CodeLocation| {
      f(a.downcast_ref().expect("found by TypeId")).to_expr(loc)
    };
    let prev = self.handlers.insert(TypeId::of::<T>(), Box::new(cb));
    assert!(prev.is_none(), "A handler for this type is already registered");
  }

  /// Add a handler function to interpret a command and select the continuation.
  /// See [HandlerTable#register] for a procedural option.
  pub fn with<T: 'static>(
    mut self,
    f: impl FnMut(&T) -> ExternResult<Expr> + 'a,
  ) -> Self {
    self.register(f);
    self
  }

  /// Find and execute the corresponding handler for this type
  pub fn dispatch(
    &mut self,
    arg: &dyn Atomic,
    loc: CodeLocation,
  ) -> Option<Expr> {
    (self.handlers.get_mut(&arg.as_any_ref().type_id()))
      .map(|f| f(arg.as_any_ref(), loc))
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

/// [run] orchid code, executing any commands it returns using the specified
/// [Handler]s.
pub fn run_handler(
  mut state: Expr,
  handlers: &mut HandlerTable,
  mut ctx: RunContext,
) -> Result<Halt, RunError> {
  loop {
    let halt = run(state, ctx.clone())?;
    state = halt.state;
    ctx.use_gas(halt.gas.unwrap_or(0));
    let state_cls = state.cls();
    if let Clause::Atom(Atom(a)) = &*state_cls {
      if let Some(res) = handlers.dispatch(a.as_ref(), state.location()) {
        drop(state_cls);
        state = res;
        continue;
      }
    }
    if halt.inert || ctx.no_gas() {
      drop(state_cls);
      break Ok(Halt { gas: ctx.gas, inert: halt.inert, state });
    }
  }
}
