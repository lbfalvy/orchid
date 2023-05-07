use crate::foreign::Atom;
use crate::representations::Primitive;
use crate::representations::interpreted::{Clause, ExprInst};

use super::apply::apply;
use super::error::RuntimeError;
use super::context::{Context, Return};

pub fn run(expr: ExprInst, mut ctx: Context)
-> Result<Return, RuntimeError>
{
  let state = expr.try_normalize(|cls| -> Result<Clause, RuntimeError> {
    let mut i = cls.clone();
    while ctx.gas.map(|g| g > 0).unwrap_or(true) {
      match &i {
        Clause::Apply { f, x } => {
          let res = apply(f.clone(), x.clone(), ctx.clone())?;
          if ctx.is_stuck(res.gas) {return Ok(i)}
          ctx.gas = res.gas;
          i = res.state.expr().clause.clone();
        }
        Clause::P(Primitive::Atom(Atom(data))) => {
          let (clause, gas) = data.run(ctx.clone())?;
          if ctx.is_stuck(gas) {return Ok(i)}
          ctx.gas = gas;
          i = clause.clone();
        }
        Clause::Constant(c) => {
          let symval = ctx.symbols.get(c).expect("missing symbol for value");
          ctx.gas = ctx.gas.map(|g| g - 1); // cost of lookup
          i = symval.expr().clause.clone();
        }
        _ => return Ok(i)
      }
    }
    Ok(i)
  })?;
  Ok(Return { state, gas: ctx.gas })
}