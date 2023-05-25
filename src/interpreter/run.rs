use std::mem;
use std::rc::Rc;

use super::apply::apply;
use super::context::{Context, Return};
use super::error::RuntimeError;
use crate::foreign::{Atom, Atomic, AtomicReturn, ExternError};
use crate::representations::interpreted::{Clause, ExprInst};
use crate::representations::Primitive;

/// Normalize an expression using beta reduction with memoization
pub fn run(expr: ExprInst, mut ctx: Context) -> Result<Return, RuntimeError> {
  let (state, (gas, inert)) =
    expr.try_normalize(|cls| -> Result<(Clause, _), RuntimeError> {
      let mut i = cls.clone();
      while ctx.gas.map(|g| g > 0).unwrap_or(true) {
        match &i {
          Clause::Apply { f, x } => {
            let res = apply(f.clone(), x.clone(), ctx.clone())?;
            if res.inert {
              return Ok((i, (res.gas, true)));
            }
            ctx.gas = res.gas;
            i = res.state.expr().clause.clone();
          },
          Clause::P(Primitive::Atom(data)) => {
            let ret = data.run(ctx.clone())?;
            let AtomicReturn { clause, gas, inert } = ret;
            if inert {
              return Ok((i, (gas, true)));
            }
            ctx.gas = gas;
            i = clause.clone();
          },
          Clause::Constant(c) => {
            let symval = ctx.symbols.get(c).expect("missing symbol for value");
            ctx.gas = ctx.gas.map(|g| g - 1); // cost of lookup
            i = symval.expr().clause.clone();
          },
          // non-reducible
          _ => return Ok((i, (ctx.gas, true))),
        }
      }
      // out of gas
      Ok((i, (ctx.gas, false)))
    })?;
  Ok(Return { state, gas, inert })
}

/// Opaque inert data that may encode a command to a [Handler]
pub type HandlerParm = Box<dyn Atomic>;

/// Reasons why a [Handler] could not interpret a command. Convertible from
/// either variant
pub enum HandlerErr {
  /// The command was addressed to us but its execution resulted in an error
  Extern(Rc<dyn ExternError>),
  /// This handler is not applicable, either because the [HandlerParm] is not a
  /// command or because it's meant for some other handler
  NA(HandlerParm),
}
impl From<Rc<dyn ExternError>> for HandlerErr {
  fn from(value: Rc<dyn ExternError>) -> Self {
    Self::Extern(value)
  }
}
impl<T> From<T> for HandlerErr
where
  T: ExternError + 'static,
{
  fn from(value: T) -> Self {
    Self::Extern(value.into_extern())
  }
}
impl From<HandlerParm> for HandlerErr {
  fn from(value: HandlerParm) -> Self {
    Self::NA(value)
  }
}

/// Various possible outcomes of a [Handler] execution.
pub type HandlerRes = Result<ExprInst, HandlerErr>;

/// A trait for things that may be able to handle commands returned by Orchid
/// code. This trait is implemented for [FnMut(HandlerParm) -> HandlerRes] and
/// [(Handler, Handler)], users are not supposed to implement it themselves.
///
/// A handler receives an arbitrary inert [Atomic] and uses [Atomic::as_any]
/// then [std::any::Any::downcast_ref] to obtain a known type. If this fails, it
/// returns the box in [HandlerErr::NA] which will be passed to the next
/// handler.
pub trait Handler {
  /// Attempt to resolve a command with this handler.
  fn resolve(&mut self, data: HandlerParm) -> HandlerRes;

  /// If this handler isn't applicable, try the other one.
  fn or<T: Handler>(self, t: T) -> impl Handler
  where
    Self: Sized,
  {
    (self, t)
  }
}

impl<F> Handler for F
where
  F: FnMut(HandlerParm) -> HandlerRes,
{
  fn resolve(&mut self, data: HandlerParm) -> HandlerRes {
    self(data)
  }
}

impl<T: Handler, U: Handler> Handler for (T, U) {
  fn resolve(&mut self, data: HandlerParm) -> HandlerRes {
    match self.0.resolve(data) {
      Err(HandlerErr::NA(data)) => self.1.resolve(data),
      x => x,
    }
  }
}

/// [run] orchid code, executing any commands it returns using the specified
/// [Handler]s.
pub fn run_handler(
  mut expr: ExprInst,
  mut handler: impl Handler,
  mut ctx: Context,
) -> Result<Return, RuntimeError> {
  loop {
    let ret = run(expr.clone(), ctx.clone())?;
    if ret.gas == Some(0) {
      return Ok(ret);
    }
    let state_ex = ret.state.expr();
    let a = if let Clause::P(Primitive::Atom(a)) = &state_ex.clause {
      a
    } else {
      mem::drop(state_ex);
      return Ok(ret);
    };
    let boxed = a.clone().0;
    expr = match handler.resolve(boxed) {
      Ok(expr) => expr,
      Err(HandlerErr::Extern(ext)) => Err(ext)?,
      Err(HandlerErr::NA(atomic)) =>
        return Ok(Return {
          gas: ret.gas,
          inert: ret.inert,
          state: Clause::P(Primitive::Atom(Atom(atomic))).wrap(),
        }),
    };
    ctx.gas = ret.gas;
  }
}
