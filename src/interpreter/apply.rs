use super::context::Context;
use super::error::RuntimeError;
use super::Return;
use crate::foreign::AtomicReturn;
use crate::representations::interpreted::{Clause, ExprInst};
use crate::representations::{PathSet, Primitive};
use crate::utils::Side;

/// Process the clause at the end of the provided path. Note that paths always
/// point to at least one target. Note also that this is not cached as a
/// normalization step in the intermediate expressions.
fn map_at<E>(
  path: &[Side],
  source: ExprInst,
  mapper: &mut impl FnMut(&Clause) -> Result<Clause, E>,
) -> Result<ExprInst, E> {
  source
    .try_update(|value, _loc| {
      // Pass right through lambdas
      if let Clause::Lambda { args, body } = value {
        return Ok((
          Clause::Lambda {
            args: args.clone(),
            body: map_at(path, body.clone(), mapper)?,
          },
          (),
        ));
      }
      // If the path ends here, process the next (non-lambda) node
      let (head, tail) = if let Some(sf) = path.split_first() {
        sf
      } else {
        return Ok((mapper(value)?, ()));
      };
      // If it's an Apply, execute the next step in the path
      if let Clause::Apply { f, x } = value {
        return Ok((
          match head {
            Side::Left => Clause::Apply {
              f: map_at(tail, f.clone(), mapper)?,
              x: x.clone(),
            },
            Side::Right => Clause::Apply {
              f: f.clone(),
              x: map_at(tail, x.clone(), mapper)?,
            },
          },
          (),
        ));
      }
      panic!("Invalid path")
    })
    .map(|p| p.0)
}

/// TODO replace when `!` gets stabilized
#[derive(Debug)]
enum Never {}

/// Replace the [Clause::LambdaArg] placeholders at the ends of the [PathSet]
/// with the value in the body. Note that a path may point to multiple
/// placeholders.
fn substitute(paths: &PathSet, value: Clause, body: ExprInst) -> ExprInst {
  let PathSet { steps, next } = paths;
  map_at(steps, body, &mut |checkpoint| -> Result<Clause, Never> {
    match (checkpoint, next) {
      (Clause::Lambda { .. }, _) => unreachable!("Handled by map_at"),
      (Clause::Apply { f, x }, Some((left, right))) => Ok(Clause::Apply {
        f: substitute(left, value.clone(), f.clone()),
        x: substitute(right, value.clone(), x.clone()),
      }),
      (Clause::LambdaArg, None) => Ok(value.clone()),
      (_, None) => {
        panic!("Substitution path ends in something other than LambdaArg")
      },
      (_, Some(_)) => {
        panic!("Substitution path leads into something other than Apply")
      },
    }
  })
  .unwrap()
}

/// Apply a function-like expression to a parameter.
pub fn apply(
  f: ExprInst,
  x: ExprInst,
  ctx: Context,
) -> Result<Return, RuntimeError> {
  let (state, (gas, inert)) = f.try_update(|clause, loc| match clause {
    // apply an ExternFn or an internal function
    Clause::P(Primitive::ExternFn(f)) => {
      let clause =
        f.apply(x, ctx.clone()).map_err(|e| RuntimeError::Extern(e))?;
      Ok((clause, (ctx.gas.map(|g| g - 1), false)))
    },
    Clause::Lambda { args, body } => Ok(if let Some(args) = args {
      let x_cls = x.expr().clause.clone();
      let new_xpr_inst = substitute(args, x_cls, body.clone());
      let new_xpr = new_xpr_inst.expr();
      // cost of substitution
      // XXX: should this be the number of occurrences instead?
      (new_xpr.clause.clone(), (ctx.gas.map(|x| x - 1), false))
    } else {
      (body.expr().clause.clone(), (ctx.gas, false))
    }),
    Clause::Constant(name) =>
      if let Some(sym) = ctx.symbols.get(name) {
        Ok((Clause::Apply { f: sym.clone(), x }, (ctx.gas, false)))
      } else {
        Err(RuntimeError::MissingSymbol(name.clone(), loc.clone()))
      },
    Clause::P(Primitive::Atom(atom)) => {
      // take a step in expanding atom
      let AtomicReturn { clause, gas, inert } = atom.run(ctx.clone())?;
      Ok((Clause::Apply { f: clause.wrap(), x }, (gas, inert)))
    },
    Clause::Apply { f: fun, x: arg } => {
      // take a step in resolving pre-function
      let ret = apply(fun.clone(), arg.clone(), ctx.clone())?;
      let Return { state, inert, gas } = ret;
      Ok((Clause::Apply { f: state, x }, (gas, inert)))
    },
    _ => Err(RuntimeError::NonFunctionApplication(f.clone())),
  })?;
  Ok(Return { state, gas, inert })
}
