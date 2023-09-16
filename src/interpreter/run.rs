use super::apply::apply;
use super::context::{Context, Return};
use super::error::RuntimeError;
use crate::foreign::AtomicReturn;
use crate::representations::interpreted::{Clause, ExprInst};
use crate::representations::Primitive;

/// Normalize an expression using beta reduction with memoization
pub fn run(expr: ExprInst, mut ctx: Context) -> Result<Return, RuntimeError> {
  let (state, (gas, inert)) = expr.try_normalize(
    |mut cls, loc| -> Result<(Clause, _), RuntimeError> {
      while ctx.gas.map(|g| g > 0).unwrap_or(true) {
        match cls {
          Clause::Apply { f, x } => {
            let res = apply(f, x, ctx.clone())?;
            if res.inert {
              return Ok((res.state.expr_val().clause, (res.gas, true)));
            }
            ctx.gas = res.gas;
            cls = res.state.expr().clause.clone();
          },
          Clause::P(Primitive::Atom(data)) => {
            let AtomicReturn { clause, gas, inert } = data.run(ctx.clone())?;
            if inert {
              return Ok((clause, (gas, true)));
            }
            ctx.gas = gas;
            cls = clause;
          },
          Clause::Constant(c) => {
            let symval = (ctx.symbols.get(&c)).ok_or_else(|| {
              RuntimeError::MissingSymbol(c.clone(), loc.clone())
            })?;
            ctx.gas = ctx.gas.map(|g| g - 1); // cost of lookup
            cls = symval.expr().clause.clone();
          },
          // non-reducible
          _ => return Ok((cls, (ctx.gas, true))),
        }
      }
      // out of gas
      Ok((cls, (ctx.gas, false)))
    },
  )?;
  Ok(Return { state, gas, inert })
}
