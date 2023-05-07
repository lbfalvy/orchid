use crate::foreign::Atom;
use crate::representations::Primitive;
use crate::representations::PathSet;
use crate::representations::interpreted::{ExprInst, Clause};
use crate::utils::Side;

use super::Return;
use super::error::RuntimeError;
use super::context::Context;

/// Process the clause at the end of the provided path.
/// Note that paths always point to at least one target.
/// Note also that this is not cached as a normalization step in the
/// intermediate expressions.
fn map_at<E>(
  path: &[Side], source: ExprInst,
  mapper: &mut impl FnMut(&Clause) -> Result<Clause, E>
) -> Result<ExprInst, E> {
  source.try_update(|value| {
    // Pass right through lambdas
    if let Clause::Lambda { args, body } = value {
      return Ok(Clause::Lambda {
        args: args.clone(),
        body: map_at(path, body.clone(), mapper)?
      })
    }
    // If the path ends here, process the next (non-lambda) node
    let (head, tail) = if let Some(sf) = path.split_first() {sf} else {
      return Ok(mapper(value)?)
    };
    // If it's an Apply, execute the next step in the path
    if let Clause::Apply { f, x } = value {
      return Ok(match head {
        Side::Left => Clause::Apply {
          f: map_at(tail, f.clone(), mapper)?,
          x: x.clone(),
        },
        Side::Right => Clause::Apply {
          f: f.clone(),
          x: map_at(tail, x.clone(), mapper)?,
        }
      })
    }
    panic!("Invalid path")
  })
}

fn substitute(paths: &PathSet, value: Clause, body: ExprInst) -> ExprInst {
  let PathSet{ steps, next } = paths;
  map_at(&steps, body, &mut |checkpoint| -> Result<Clause, !> {
    match (checkpoint, next) {
      (Clause::Lambda{..}, _) =>  unreachable!("Handled by map_at"),
      (Clause::Apply { f, x }, Some((left, right))) => Ok(Clause::Apply {
        f: substitute(&left, value.clone(), f.clone()),
        x: substitute(&right, value.clone(), x.clone()),
      }),
      (Clause::LambdaArg, None) => Ok(value.clone()),
      (_, None) => panic!("Substitution path ends in something other than LambdaArg"),
      (_, Some(_)) => panic!("Substitution path leads into something other than Apply"),
    }
  }).into_ok()
}

/// Apply a function-like expression to a parameter.
/// If any work is being done, gas will be deducted.
pub fn apply(
  f: ExprInst, x: ExprInst, mut ctx: Context
) -> Result<Return, RuntimeError> {
  let state = f.clone().try_update(|clause| match clause {
    // apply an ExternFn or an internal function
    Clause::P(Primitive::ExternFn(f)) => {
      let (clause, gas) = f.apply(x, ctx.clone())
        .map_err(|e| RuntimeError::Extern(e))?;
      ctx.gas = gas.map(|g| g - 1); // cost of extern call
      Ok(clause)
    }
    Clause::Lambda{args, body} => Ok(if let Some(args) = args {
      let x_cls = x.expr().clause.clone();
      let new_xpr_inst = substitute(args, x_cls, body.clone());
      let new_xpr = new_xpr_inst.expr();
      // cost of substitution
      // XXX: should this be the number of occurrences instead?
      ctx.gas = ctx.gas.map(|x| x - 1);
      new_xpr.clause.clone()
    } else {body.expr().clause.clone()}),
    Clause::Constant(name) => {
      let symval = ctx.symbols.get(name).expect("missing symbol for function").clone();
      ctx.gas = ctx.gas.map(|x| x - 1); // cost of lookup
      Ok(Clause::Apply { f: symval, x, })
    }
    Clause::P(Primitive::Atom(Atom(atom))) => { // take a step in expanding atom
      let (clause, gas) = atom.run(ctx.clone())?;
      ctx.gas = gas.map(|x| x - 1); // cost of dispatch
      Ok(Clause::Apply { f: clause.wrap(), x })
    },
    Clause::Apply{ f: fun, x: arg } => { // take a step in resolving pre-function
      let res = apply(fun.clone(), arg.clone(), ctx.clone())?;
      ctx.gas = res.gas; // if work has been done, it has been paid
      Ok(Clause::Apply{ f: res.state, x })
    },
    _ => Err(RuntimeError::NonFunctionApplication(f.clone()))
  })?;
  Ok(Return { state, gas: ctx.gas })
}