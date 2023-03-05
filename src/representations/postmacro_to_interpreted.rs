use std::{rc::Rc, fmt::Display};

use crate::utils::Side;

use super::{postmacro, interpreted, path_set::PathSet};

fn collect_paths_expr_rec(expr: &postmacro::Expr, depth: usize) -> Option<PathSet> {
  collect_paths_cls_rec(&expr.0, depth)
}

fn collect_paths_cls_rec(cls: &postmacro::Clause, depth: usize) -> Option<PathSet> {
  match cls {
    postmacro::Clause::P(_) | postmacro::Clause::Auto(..) | postmacro::Clause::AutoArg(_)
      | postmacro::Clause::Explicit(..) => None,
    postmacro::Clause::LambdaArg(h) => if *h != depth {None} else {
      Some(PathSet{ next: None, steps: Rc::new(vec![]) })
    }
    postmacro::Clause::Lambda(_, b) => collect_paths_expr_rec(b, depth + 1),
    postmacro::Clause::Apply(f, x) => {
      let f_opt = collect_paths_expr_rec(f, depth);
      let x_opt = collect_paths_expr_rec(x, depth);
      match (f_opt, x_opt) {
        (Some(f_refs), Some(x_refs)) => Some(f_refs + x_refs),
        (Some(f_refs), None) => Some(f_refs + Side::Left),
        (None, Some(x_refs)) => Some(x_refs + Side::Right),
        (None, None) => None
      }
    }
  }
}

#[derive(Clone)]
pub enum Error {
  /// Auto, Explicit and AutoArg are unsupported
  GenericMention,
  /// Type annotations are unsupported
  ExplicitType
}

impl Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::ExplicitType => write!(f, "Type annotations are unsupported in the interpreter"),
      Self::GenericMention
        => write!(f, "The interpreter is typeless and therefore can't resolve generics")
    }
  }
}

pub fn clause_rec(cls: &postmacro::Clause) -> Result<interpreted::Clause, Error> {
  match cls {
    postmacro::Clause::P(p) => Ok(interpreted::Clause::P(p.clone())),
    postmacro::Clause::Explicit(..) | postmacro::Clause::AutoArg(..) | postmacro::Clause::Auto(..)
      => Err(Error::GenericMention),
    postmacro::Clause::Apply(f, x) => Ok(interpreted::Clause::Apply {
      f: Rc::new(expr_rec(f.as_ref())?),
      x: Rc::new(expr_rec(x.as_ref())?),
      id: 0
    }),
    postmacro::Clause::Lambda(typ, body) => if typ.len() != 0 {Err(Error::ExplicitType)} else {
      Ok(interpreted::Clause::Lambda {
        args: collect_paths_expr_rec(body, 0),
        body: Rc::new(expr_rec(body)?)
      })
    },
    postmacro::Clause::LambdaArg(_) => Ok(interpreted::Clause::LambdaArg)
  }
}

pub fn expr_rec(expr: &postmacro::Expr) -> Result<interpreted::Clause, Error> {
  let postmacro::Expr(c, t) = expr;
  if t.len() != 0 {Err(Error::ExplicitType)}
  else {clause_rec(c)}
}