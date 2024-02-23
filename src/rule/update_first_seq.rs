use std::iter;
use std::rc::Rc;

use super::matcher::RuleExpr;
use crate::parse::parsed::{Clause, Expr};

/// Traverse the tree, calling pred on every sibling list until it returns
/// some vec then replace the sibling list with that vec and return true
/// return false if pred never returned some
#[must_use]
pub fn exprv<F: FnMut(Rc<Vec<RuleExpr>>) -> Option<Rc<Vec<RuleExpr>>>>(
  input: Rc<Vec<RuleExpr>>,
  pred: &mut F,
) -> Option<Rc<Vec<RuleExpr>>> {
  if let Some(v) = pred(input.clone()) {
    return Some(v);
  }
  replace_first(input.as_ref(), |ex| expr(ex, pred)).map(|i| Rc::new(i.collect()))
}

#[must_use]
pub fn expr<F: FnMut(Rc<Vec<RuleExpr>>) -> Option<Rc<Vec<RuleExpr>>>>(
  input: &RuleExpr,
  pred: &mut F,
) -> Option<RuleExpr> {
  clause(&input.value, pred).map(|value| Expr { value, range: input.range.clone() })
}

#[must_use]
pub fn clause<F: FnMut(Rc<Vec<RuleExpr>>) -> Option<Rc<Vec<RuleExpr>>>>(
  c: &Clause,
  pred: &mut F,
) -> Option<Clause> {
  match c {
    Clause::Atom(_) | Clause::Placeh { .. } | Clause::Name { .. } => None,
    Clause::Lambda(arg, body) =>
      if let Some(arg) = exprv(arg.clone(), pred) {
        Some(Clause::Lambda(arg, body.clone()))
      } else {
        exprv(body.clone(), pred).map(|body| Clause::Lambda(arg.clone(), body))
      },
    Clause::S(c, body) => Some(Clause::S(*c, exprv(body.clone(), pred)?)),
  }
}

/// Iterate over a sequence with the first element updated for which the
/// function returns Some(), but only if there is such an element.
pub fn replace_first<T: Clone, F: FnMut(&T) -> Option<T>>(
  slice: &[T],
  mut f: F,
) -> Option<impl Iterator<Item = T> + '_> {
  for i in 0..slice.len() {
    if let Some(new) = f(&slice[i]) {
      let subbed_iter =
        slice[0..i].iter().cloned().chain(iter::once(new)).chain(slice[i + 1..].iter().cloned());
      return Some(subbed_iter);
    }
  }
  None
}
