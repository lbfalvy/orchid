use mappable_rc::Mrc;

use crate::{ast::{Expr, Clause}, utils::{replace_first, to_mrc_slice}};

/// Traverse the tree, calling pred on every sibling list until it returns some vec
/// then replace the sibling list with that vec and return true
/// return false if pred never returned some
pub fn exprv<F>(input: Mrc<[Expr]>, pred: &mut F) -> Option<Mrc<[Expr]>>
where F: FnMut(Mrc<[Expr]>) -> Option<Mrc<[Expr]>> {
  if let o@Some(_) = pred(Mrc::clone(&input)) {return o} 
  replace_first(input.as_ref(), |ex| expr(ex, pred))
    .map(|i| to_mrc_slice(i.collect()))
}

pub fn expr<F>(Expr(cls, typ): &Expr, pred: &mut F) -> Option<Expr>
where F: FnMut(Mrc<[Expr]>) -> Option<Mrc<[Expr]>> {
  if let Some(t) = clausev(Mrc::clone(typ), pred) {return Some(Expr(cls.clone(), t))}
  if let Some(c) = clause(cls, pred) {return Some(Expr(c, Mrc::clone(typ)))}
  None
}

pub fn clausev<F>(input: Mrc<[Clause]>, pred: &mut F) -> Option<Mrc<[Clause]>>
where F: FnMut(Mrc<[Expr]>) -> Option<Mrc<[Expr]>> {
  replace_first(input.as_ref(), |c| clause(c, pred))
    .map(|i| to_mrc_slice(i.collect()))
}

pub fn clause<F>(c: &Clause, pred: &mut F) -> Option<Clause>
where F: FnMut(Mrc<[Expr]>) -> Option<Mrc<[Expr]>> {
  match c {
    Clause::P(_) | Clause::Placeh {..} | Clause::Name {..} => None,
    Clause::Lambda(n, typ, body) => {
      if let Some(b) = exprv(Mrc::clone(body), pred) {
        return Some(Clause::Lambda(n.clone(), Mrc::clone(typ), b))
      }
      if let Some(t) = exprv(Mrc::clone(typ), pred) {
        return Some(Clause::Lambda(n.clone(), t, Mrc::clone(body)))
      }
      None
    }
    Clause::Auto(n, typ, body) => {
      if let Some(b) = exprv(Mrc::clone(body), pred) {
        return Some(Clause::Auto(n.clone(), Mrc::clone(typ), b))
      }
      if let Some(t) = exprv(Mrc::clone(typ), pred) {
        return Some(Clause::Auto(n.clone(), t, Mrc::clone(body)))
      }
      None
    }
    Clause::S(c, body) => Some(Clause::S(*c, exprv(Mrc::clone(body), pred)?)),
    Clause::Explicit(t) => Some(Clause::Explicit(Mrc::new(expr(t, pred)?)))
  }
}