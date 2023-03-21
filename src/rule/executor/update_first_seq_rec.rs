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

pub fn expr<F>(Expr(cls, typ): &Expr, pred: &mut F) -> Option<Expr>
where F: FnMut(Rc<Vec<Expr>>) -> Option<Rc<Vec<Expr>>> {
  if let Some(t) = clausev(typ.clone(), pred) {return Some(Expr(cls.clone(), t))}
  if let Some(c) = clause(cls, pred) {return Some(Expr(c, typ.clone()))}
  None
}

pub fn clausev<F>(input: Rc<Vec<Clause>>, pred: &mut F) -> Option<Rc<Vec<Clause>>>
where F: FnMut(Rc<Vec<Expr>>) -> Option<Rc<Vec<Expr>>> {
  replace_first(input.as_ref(), |c| clause(c, pred))
    .map(|i| Rc::new(i.collect()))
}

pub fn clause<F>(c: &Clause, pred: &mut F) -> Option<Clause>
where F: FnMut(Rc<Vec<Expr>>) -> Option<Rc<Vec<Expr>>> {
  match c {
    Clause::P(_) | Clause::Placeh {..} | Clause::Name {..} => None,
    Clause::Lambda(n, typ, body) => {
      if let Some(b) = exprv(body.clone(), pred) {
        return Some(Clause::Lambda(n.clone(), typ.clone(), b))
      }
      if let Some(t) = exprv(typ.clone(), pred) {
        return Some(Clause::Lambda(n.clone(), t, body.clone()))
      }
      None
    }
    Clause::Auto(n, typ, body) => {
      if let Some(b) = exprv(body.clone(), pred) {
        return Some(Clause::Auto(n.clone(), typ.clone(), b))
      }
      if let Some(t) = exprv(typ.clone(), pred) {
        return Some(Clause::Auto(n.clone(), t, body.clone()))
      }
      None
    }
    Clause::S(c, body) => Some(Clause::S(*c, exprv(body.clone(), pred)?)),
    Clause::Explicit(t) => Some(Clause::Explicit(Rc::new(expr(t, pred)?)))
  }
}