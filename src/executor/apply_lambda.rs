use itertools::Itertools;
use mappable_rc::Mrc;

use crate::utils::{collect_to_mrc, to_mrc_slice};

use crate::representations::typed::{Clause, Expr};

#[derive(Clone)]
struct Application<'a> {
  id: u64,
  value: &'a Expr,
  types: bool
}

// pub fn apply_lambda(app: Application, body: Expr) -> Expr {
//   apply_lambda_expr_rec(id, value, body)
//     .unwrap_or(body)
// }

fn apply_lambda_expr_rec(
  app@Application{ id, types, value }: Application, expr: &Expr
) -> Option<Expr> {
  let Expr(clause, typ) = expr;
  match clause {
    Clause::LambdaArg(arg_id) | Clause::AutoArg(arg_id) if *arg_id == id => {
      let full_typ = 
        value.1.iter()
        .chain(typ.iter())
        .cloned().collect_vec();
      Some(Expr(value.0.to_owned(), full_typ))
    }
    cl => {
      let new_cl = apply_lambda_clause_rec(app, cl);
      let new_typ = if !types {None} else {
        typ.
      }
    }
  }
}

fn apply_lambda_clause_rec(
  app: Application, clause: &Clause
) -> Option<Clause> {
  match clause {
    // Only element actually manipulated
    Clause::LambdaArg(_) | Clause::AutoArg(_) => None,
    // Traverse, yield Some if either had changed.
    Clause::Apply(f, x) => {
      let new_f = apply_lambda_expr_rec(app, f.as_ref());
      let new_x = apply_lambda_expr_rec(app, x.as_ref());
      match (new_f, new_x) { // Mind the shadows
        (None, None) => None,
        (None, Some(x)) => Some(Clause::Apply(f.clone(), Box::new(x))),
        (Some(f), None) => Some(Clause::Apply(Box::new(f), x.clone())),
        (Some(f), Some(x)) => Some(Clause::Apply(Box::new(f), Box::new(x)))
      }
    },
    Clause::Lambda(own_id, t, b) => apply_lambda__traverse_param(id, value, own_id, t, b, Clause::Lambda),
    Clause::Auto(own_id, t, b) => apply_lambda__traverse_param(id, value, own_id, t, b, Clause::Auto),
    // Leaf nodes
    Clause::Atom(_) | Clause::ExternFn(_) | Clause::Literal(_) => None
  }
}

fn apply_lambda__traverse_param(
  id: u64, value: Mrc<Expr>,
  own_id: u64, typ: Mrc<[Clause]>, b: Mrc<Expr>,
  wrap: impl Fn(u64, Mrc<[Clause]>, Mrc<Expr>) -> Clause
) -> Option<Clause> {
  let any_t = false;
  let mut t_acc = vec![];
  for t in typ.iter() {
    let newt = apply_lambda_clause_rec(id, Mrc::clone(&value), t.clone());
    any_t |= newt.is_some();
    t_acc.push(newt.unwrap_or_else(|| t.clone()))
  }
  // Respect shadowing
  let new_b = if own_id == id {None} else {
    apply_lambda_expr_rec(id, value, Mrc::clone(&b))
  };
  if any_t { // mind the shadows
    let typ = to_mrc_slice(t_acc);
    if let Some(b) = new_b {
      Some(wrap(own_id, typ, b))
    } else {Some(wrap(own_id, typ, b))}
  } else if let Some(b) = new_b {
    Some(wrap(own_id, typ, b))
  } else {Some(wrap(own_id, typ, b))}
}