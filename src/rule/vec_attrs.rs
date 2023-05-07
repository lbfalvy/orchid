use crate::interner::Token;
use crate::ast::{Expr, PHClass, Placeholder, Clause};

/// Returns the name, priority and nonzero of the expression if it is
/// a vectorial placeholder
pub fn vec_attrs(expr: &Expr) -> Option<(Token<String>, u64, bool)> {
  if let Clause::Placeh(
    Placeholder{ class: PHClass::Vec{ prio, nonzero }, name }
  ) = expr.value {Some((name, prio, nonzero))} else {None}
}