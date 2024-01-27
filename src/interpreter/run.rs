use std::collections::VecDeque;

use hashbrown::HashMap;

use super::apply::apply;
use super::context::{Halt, RunContext};
use super::error::RunError;
use super::nort::{Clause, Expr};
use crate::foreign::atom::AtomicReturn;
use crate::foreign::error::ExternResult;
use crate::location::CodeLocation;
use crate::name::Sym;
use crate::utils::pure_seq::pushed;

/// Information about a normalization run presented to an atom
#[derive(Clone)]
pub struct RunData<'a> {
  /// Location of the atom
  pub location: CodeLocation,
  /// Information about the execution
  pub ctx: RunContext<'a>,
}

#[derive(Debug)]
pub struct Interrupted {
  stack: Vec<Expr>,
}
impl Interrupted {
  pub fn resume(self, ctx: RunContext) -> Result<Halt, RunError> {
    run_stack(self.stack, ctx)
  }
}

/// Normalize an expression using beta reduction with memoization
pub fn run(mut expr: Expr, mut ctx: RunContext) -> Result<Halt, RunError> {
  run_stack(vec![expr], ctx)
}

fn run_stack(
  mut stack: Vec<Expr>,
  mut ctx: RunContext,
) -> Result<Halt, RunError> {
  let mut expr = stack.pop().expect("Empty stack");
  loop {
    if ctx.no_gas() {
      return Err(RunError::Interrupted(Interrupted {
        stack: pushed(stack, expr),
      }));
    }
    let (next_clsi, inert) = expr.clause.try_normalize(|mut cls| {
      loop {
        if ctx.no_gas() {
          return Ok((cls, false));
        }
        match cls {
          cls @ Clause::Identity(_) => return Ok((cls, false)),
          // TODO:
          // - unfuck nested loop
          // - inline most of [apply] to eliminate recursion step
          Clause::Apply { f, x } => {
            if x.is_empty() {
              return Ok((f.clause.into_cls(), false));
            }
            let (gas, clause) = apply(f, x, ctx.clone())?;
            if ctx.gas.is_some() {
              ctx.gas = gas;
            }
            cls = clause;
          },
          Clause::Atom(data) => {
            let run = RunData { ctx: ctx.clone(), location: expr.location() };
            let atomic_ret = data.run(run)?;
            if ctx.gas.is_some() {
              ctx.gas = atomic_ret.gas;
            }
            if atomic_ret.inert {
              return Ok((atomic_ret.clause, true));
            }
            cls = atomic_ret.clause;
          },
          Clause::Constant(c) => {
            let symval = (ctx.symbols.get(&c)).ok_or_else(|| {
              RunError::MissingSymbol(c.clone(), expr.location())
            })?;
            ctx.gas = ctx.gas.map(|g| g - 1); // cost of lookup
            cls = Clause::Identity(symval.clause.clone());
          },
          // non-reducible
          c => return Ok((c, true)),
        };
      }
    })?;
    expr.clause = next_clsi;
    if inert {
      match stack.pop() {
        Some(e) => expr = e,
        None => return Ok(Halt { state: expr, gas: ctx.gas, inert }),
      }
    }
  }
}
