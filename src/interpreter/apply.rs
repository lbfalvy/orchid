use never::Never;

use super::context::{RunEnv, RunParams};
use super::nort::{Clause, ClauseInst, Expr};
use super::path_set::{PathSet, Step};
use crate::foreign::atom::CallData;
use crate::foreign::error::RTResult;

/// Process the clause at the end of the provided path. Note that paths always
/// point to at least one target. Note also that this is not cached as a
/// normalization step in the intermediate expressions.
fn map_at<E>(
  mut path: impl Iterator<Item = Step>,
  source: &Clause,
  mapper: &mut impl FnMut(&Clause) -> Result<Clause, E>,
) -> Result<Clause, E> {
  // Pass through some unambiguous wrapper clauses
  match source {
    Clause::Identity(alt) => return map_at(path, &alt.cls_mut(), mapper),
    Clause::Lambda { args, body: Expr { location: b_loc, clause } } =>
      return Ok(Clause::Lambda {
        args: args.clone(),
        body: Expr {
          clause: map_at(path, &clause.cls_mut(), mapper)?.into_inst(),
          location: b_loc.clone(),
        },
      }),
    _ => (),
  }
  Ok(match (source, path.next()) {
    (Clause::Lambda { .. } | Clause::Identity(_), _) => unreachable!("Handled above"),
    // If the path ends and this isn't a lambda, process it
    (val, None) => mapper(val)?,
    // If it's an Apply, execute the next step in the path
    (Clause::Apply { f, x }, Some(head)) => {
      let proc = |x: &Expr| Ok(map_at(path, &x.cls_mut(), mapper)?.into_expr(x.location()));
      match head {
        None => Clause::Apply { f: proc(f)?, x: x.clone() },
        Some(n) => {
          let i = x.len() - n - 1;
          let mut argv = x.clone();
          argv[i] = proc(&x[i])?;
          Clause::Apply { f: f.clone(), x: argv }
        },
      }
    },
    (_, Some(_)) => panic!("Path leads into node that isn't Apply or Lambda"),
  })
}

/// Replace the [Clause::LambdaArg] placeholders at the ends of the [PathSet]
/// with the value in the body. Note that a path may point to multiple
/// placeholders.
#[must_use]
pub fn substitute(
  paths: &PathSet,
  value: ClauseInst,
  body: &Clause,
  on_sub: &mut impl FnMut(),
) -> Clause {
  let PathSet { steps, next } = paths;
  map_at(steps.iter().cloned(), body, &mut |chkpt| -> Result<Clause, Never> {
    match (chkpt, next) {
      (Clause::Lambda { .. } | Clause::Identity(_), _) => {
        unreachable!("Handled by map_at")
      },
      (Clause::Apply { f, x }, Some(conts)) => {
        let mut argv = x.clone();
        let f = match conts.get(&None) {
          None => f.clone(),
          Some(sp) => substitute(sp, value.clone(), &f.cls_mut(), on_sub).into_expr(f.location()),
        };
        for (i, old) in argv.iter_mut().rev().enumerate() {
          if let Some(sp) = conts.get(&Some(i)) {
            let tmp = substitute(sp, value.clone(), &old.cls_mut(), on_sub);
            *old = tmp.into_expr(old.location());
          }
        }
        Ok(Clause::Apply { f, x: argv })
      },
      (Clause::LambdaArg, None) => {
        on_sub();
        Ok(Clause::Identity(value.clone()))
      },
      (_, None) => panic!("Argument path must point to LambdaArg"),
      (_, Some(_)) => panic!("Argument path can only fork at Apply"),
    }
  })
  .unwrap_or_else(|e| match e {})
}

pub(super) fn apply_as_atom(
  f: Expr,
  arg: Expr,
  env: &RunEnv,
  params: &mut RunParams,
) -> RTResult<Clause> {
  let call = CallData { location: f.location(), arg, env, params };
  match f.clause.try_unwrap() {
    Ok(clause) => match clause {
      Clause::Atom(atom) => Ok(atom.apply(call)?),
      _ => panic!("Not an atom"),
    },
    Err(clsi) => match &mut *clsi.cls_mut() {
      Clause::Atom(atom) => Ok(atom.apply_mut(call)?),
      _ => panic!("Not an atom"),
    },
  }
}
