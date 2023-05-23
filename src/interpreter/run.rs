use std::mem;
use std::rc::Rc;

use crate::foreign::{AtomicReturn, Atomic, ExternError, Atom};
use crate::representations::Primitive;
use crate::representations::interpreted::{Clause, ExprInst};

use super::apply::apply;
use super::error::RuntimeError;
use super::context::{Context, Return};

pub fn run(
  expr: ExprInst,
  mut ctx: Context
) -> Result<Return, RuntimeError> {
  let (state, (gas, inert)) = expr.try_normalize(|cls| -> Result<(Clause, _), RuntimeError> {
    let mut i = cls.clone();
    while ctx.gas.map(|g| g > 0).unwrap_or(true) {
      match &i {
        Clause::Apply { f, x } => {
          let res = apply(f.clone(), x.clone(), ctx.clone())?;
          if res.inert {return Ok((i, (res.gas, true)))}
          ctx.gas = res.gas;
          i = res.state.expr().clause.clone();
        }
        Clause::P(Primitive::Atom(data)) => {
          let ret = data.run(ctx.clone())?;
          let AtomicReturn { clause, gas, inert } = ret;
          if inert {return Ok((i, (gas, true)))}
          ctx.gas = gas;
          i = clause.clone();
        }
        Clause::Constant(c) => {
          let symval = ctx.symbols.get(c).expect("missing symbol for value");
          ctx.gas = ctx.gas.map(|g| g - 1); // cost of lookup
          i = symval.expr().clause.clone();
        }
        // non-reducible
        _ => return Ok((i, (ctx.gas, true)))
      }
    }
    // out of gas
    Ok((i, (ctx.gas, false)))
  })?;
  Ok(Return { state, gas, inert })
}

pub type HandlerParm = Box<dyn Atomic>;
pub type HandlerRes = Result<
  Result<ExprInst, Rc<dyn ExternError>>,
  HandlerParm
>;

pub trait Handler {
  fn resolve(&mut self, data: HandlerParm) -> HandlerRes;

  fn then<T: Handler>(self, t: T) -> impl Handler where Self: Sized {
    Pair(self, t)
  }
}

impl<F> Handler for F where F: FnMut(HandlerParm) -> HandlerRes {
  fn resolve(&mut self, data: HandlerParm) -> HandlerRes {
    self(data)
  }
}

pub struct Pair<T, U>(T, U);

impl<T: Handler, U: Handler> Handler for Pair<T, U> {
  fn resolve(&mut self, data: HandlerParm) -> HandlerRes {
    match self.0.resolve(data) {
      Ok(out) => Ok(out),
      Err(data) => self.1.resolve(data)
    }
  }
}

pub fn run_handler(
  mut expr: ExprInst,
  mut handler: impl Handler,
  mut ctx: Context
) -> Result<Return, RuntimeError> {
  loop {
    let ret = run(expr.clone(), ctx.clone())?;
    if ret.gas == Some(0) {
      return Ok(ret)
    }
    let state_ex = ret.state.expr();
    let a = if let Clause::P(Primitive::Atom(a)) = &state_ex.clause {a}
    else {
      mem::drop(state_ex);
      return Ok(ret)
    };
    let boxed = a.clone().0;
    expr = match handler.resolve(boxed) {
      Ok(r) => r.map_err(RuntimeError::Extern)?,
      Err(e) => return Ok(Return{
        gas: ret.gas,
        inert: ret.inert,
        state: Clause::P(Primitive::Atom(Atom(e))).wrap()
      })
    };
    ctx.gas = ret.gas;
  }
}