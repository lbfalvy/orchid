use std::rc::Rc;

use super::matcher::RuleExpr;
use crate::ast::{Clause, Expr};
use crate::utils::replace_first;
use crate::Sym;

/// Traverse the tree, calling pred on every sibling list until it returns
/// some vec then replace the sibling list with that vec and return true
/// return false if pred never returned some
pub fn exprv<F: FnMut(Rc<Vec<RuleExpr>>) -> Option<Rc<Vec<RuleExpr>>>>(
  input: Rc<Vec<RuleExpr>>,
  pred: &mut F,
) -> Option<Rc<Vec<RuleExpr>>> {
  if let Some(v) = pred(input.clone()) {
    return Some(v);
  }
  replace_first(input.as_ref(), |ex| expr(ex, pred))
    .map(|i| Rc::new(i.collect()))
}

pub fn expr<F: FnMut(Rc<Vec<RuleExpr>>) -> Option<Rc<Vec<RuleExpr>>>>(
  input: &RuleExpr,
  pred: &mut F,
) -> Option<RuleExpr> {
  clause(&input.value, pred)
    .map(|value| Expr { value, location: input.location.clone() })
}

pub fn clause<F: FnMut(Rc<Vec<RuleExpr>>) -> Option<Rc<Vec<RuleExpr>>>>(
  c: &Clause<Sym>,
  pred: &mut F,
) -> Option<Clause<Sym>> {
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
