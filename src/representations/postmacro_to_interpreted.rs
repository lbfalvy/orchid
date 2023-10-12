use std::sync::{Arc, Mutex};

use super::path_set::PathSet;
use super::{interpreted, postmacro};
use crate::utils::Side;

#[must_use]
fn collect_paths_expr_rec(
  expr: &postmacro::Expr,
  depth: usize,
) -> Option<PathSet> {
  collect_paths_cls_rec(&expr.value, depth)
}

fn collect_paths_cls_rec(
  cls: &postmacro::Clause,
  depth: usize,
) -> Option<PathSet> {
  match cls {
    postmacro::Clause::Atom(_) | postmacro::Clause::ExternFn(_) => None,
    postmacro::Clause::Constant(_) => None,
    postmacro::Clause::LambdaArg(h) => match *h != depth {
      true => None,
      false => Some(PathSet::pick()),
    },
    postmacro::Clause::Lambda(b) => collect_paths_expr_rec(b, depth + 1),
    postmacro::Clause::Apply(f, x) => {
      let f_opt = collect_paths_expr_rec(f, depth);
      let x_opt = collect_paths_expr_rec(x, depth);
      match (f_opt, x_opt) {
        (Some(f_refs), Some(x_refs)) => Some(f_refs + x_refs),
        (Some(f_refs), None) => Some(f_refs + Side::Left),
        (None, Some(x_refs)) => Some(x_refs + Side::Right),
        (None, None) => None,
      }
    },
  }
}

pub fn clause(cls: &postmacro::Clause) -> interpreted::Clause {
  match cls {
    postmacro::Clause::Constant(name) =>
      interpreted::Clause::Constant(name.clone()),
    postmacro::Clause::Atom(a) => interpreted::Clause::Atom(a.clone()),
    postmacro::Clause::ExternFn(fun) =>
      interpreted::Clause::ExternFn(fun.clone()),
    postmacro::Clause::Apply(f, x) =>
      interpreted::Clause::Apply { f: expr(f.as_ref()), x: expr(x.as_ref()) },
    postmacro::Clause::Lambda(body) => interpreted::Clause::Lambda {
      args: collect_paths_expr_rec(body, 0),
      body: expr(body),
    },
    postmacro::Clause::LambdaArg(_) => interpreted::Clause::LambdaArg,
  }
}

pub fn expr(expr: &postmacro::Expr) -> interpreted::ExprInst {
  interpreted::ExprInst(Arc::new(Mutex::new(interpreted::Expr {
    location: expr.location.clone(),
    clause: clause(&expr.value),
  })))
}
