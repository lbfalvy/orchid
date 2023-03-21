use std::rc::Rc;

use lasso::Spur;

use crate::ast::{Expr, Clause};

/// Replaces the first element of a name with the matching prefix from a prefix map

/// Produce a Token object for any value of Expr other than Typed.
/// Called by [#prefix] which handles Typed.
fn prefix_clause(
  expr: &Clause,
  namespace: &[Spur]
) -> Clause {
  match expr {
    Clause::S(c, v) => Clause::S(*c, Rc::new(v.iter().map(|e| {
      prefix_expr(e, namespace)
    }).collect())),
    Clause::Auto(name, typ, body) => Clause::Auto(
      name.clone(),
      Rc::new(typ.iter().map(|e| prefix_expr(e, namespace)).collect()),
      Rc::new(body.iter().map(|e| prefix_expr(e, namespace)).collect()),
    ),
    Clause::Lambda(name, typ, body) => Clause::Lambda(
      name.clone(),
      Rc::new(typ.iter().map(|e| prefix_expr(e, namespace)).collect()),
      Rc::new(body.iter().map(|e| prefix_expr(e, namespace)).collect()),
    ),
    Clause::Name(name) => Clause::Name(
      Rc::new(namespace.iter().chain(name.iter()).cloned().collect())
    ),
    x => x.clone()
  }
}

/// Produce an Expr object for any value of Expr
pub fn prefix_expr(Expr(clause, typ): &Expr, namespace: &[Spur]) -> Expr {
  Expr(
    prefix_clause(clause, namespace),
    Rc::new(typ.iter().map(|e| prefix_clause(e, namespace)).collect())
  )
}
