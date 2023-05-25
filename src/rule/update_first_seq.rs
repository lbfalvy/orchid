use std::rc::Rc;

use crate::ast::{Clause, Expr};
use crate::utils::replace_first;

/// Traverse the tree, calling pred on every sibling list until it returns
/// some vec then replace the sibling list with that vec and return true
/// return false if pred never returned some
pub fn exprv<F: FnMut(Rc<Vec<Expr>>) -> Option<Rc<Vec<Expr>>>>(
  input: Rc<Vec<Expr>>,
  pred: &mut F,
) -> Option<Rc<Vec<Expr>>> {
  if let Some(v) = pred(input.clone()) {
    return Some(v);
  }
  replace_first(input.as_ref(), |ex| expr(ex, pred))
    .map(|i| Rc::new(i.collect()))
}

pub fn expr<F: FnMut(Rc<Vec<Expr>>) -> Option<Rc<Vec<Expr>>>>(
  input: &Expr,
  pred: &mut F,
) -> Option<Expr> {
  clause(&input.value, pred)
    .map(|value| Expr { value, location: input.location.clone() })
}

pub fn clause<F: FnMut(Rc<Vec<Expr>>) -> Option<Rc<Vec<Expr>>>>(
  c: &Clause,
  pred: &mut F,
) -> Option<Clause> {
  match c {
    Clause::P(_) | Clause::Placeh { .. } | Clause::Name { .. } => None,
    Clause::Lambda(arg, body) =>
      if let Some(arg) = expr(arg.as_ref(), pred) {
        Some(Clause::Lambda(Rc::new(arg), body.clone()))
      } else {
        exprv(body.clone(), pred).map(|body| Clause::Lambda(arg.clone(), body))
      },
    Clause::S(c, body) => Some(Clause::S(*c, exprv(body.clone(), pred)?)),
  }
}
