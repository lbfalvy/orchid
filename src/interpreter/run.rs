use crate::foreign::AtomicReturn;
use crate::representations::Primitive;
use crate::representations::interpreted::{Clause, ExprInst};

use super::apply::apply;
use super::error::RuntimeError;
use super::context::{Context, Return};

pub fn run(expr: ExprInst, mut ctx: Context)
-> Result<Return, RuntimeError>
{
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