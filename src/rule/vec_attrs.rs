use super::matcher::RuleExpr;
use crate::ast::{Clause, PHClass, Placeholder};
use crate::interner::Tok;

/// Returns the name, priority and nonzero of the expression if it is
/// a vectorial placeholder
#[must_use]
pub fn vec_attrs(expr: &RuleExpr) -> Option<(Tok<String>, usize, bool)> {
  match expr.value.clone() {
    Clause::Placeh(Placeholder {
      class: PHClass::Vec { prio, nonzero },
      name,
    }) => Some((name, prio, nonzero)),
    _ => None,
  }
}
