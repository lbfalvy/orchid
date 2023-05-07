use std::rc::Rc;

use crate::utils::replace_first;
use crate::ast::{Expr, Clause};

/// Traverse the tree, calling pred on every sibling list until it returns
/// some vec then replace the sibling list with that vec and return true
/// return false if pred never returned some
pub fn exprv<F>(input: Rc<Vec<Expr>>, pred: &mut F) -> Option<Rc<Vec<Expr>>>
where F: FnMut(Rc<Vec<Expr>>) -> Option<Rc<Vec<Expr>>> {
  if let o@Some(_) = pred(input.clone()) {return o} 
  replace_first(input.as_ref(), |ex| expr(ex, pred))
    .map(|i| Rc::new(i.collect()))
}

pub fn expr<F>(input: &Expr, pred: &mut F) -> Option<Expr>
where F: FnMut(Rc<Vec<Expr>>) -> Option<Rc<Vec<Expr>>> {
  if let Some(value) = clause(&input.value, pred) {
    Some(Expr{ value, location: input.location.clone() })
  } else {None}
}

pub fn clause<F>(c: &Clause, pred: &mut F) -> Option<Clause>
where F: FnMut(Rc<Vec<Expr>>) -> Option<Rc<Vec<Expr>>> {
  match c {
    Clause::P(_) | Clause::Placeh {..} | Clause::Name {..} => None,
    Clause::Lambda(arg, body) => {
      if let Some(arg) = expr(arg.as_ref(), pred) {
        Some(Clause::Lambda(Rc::new(arg), body.clone()))
      } else if let Some(body) = exprv(body.clone(), pred) {
        Some(Clause::Lambda(arg.clone(), body))
      } else {None}
    }
    Clause::S(c, body) => Some(Clause::S(*c, exprv(body.clone(), pred)?)),
  }
}