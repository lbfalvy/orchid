use crate::ast::{Clause, Expr, PHClass, Placeholder};
use crate::interner::Tok;

/// Returns the name, priority and nonzero of the expression if it is
/// a vectorial placeholder
pub fn vec_attrs(expr: &Expr) -> Option<(Tok<String>, u64, bool)> {
  if let Clause::Placeh(Placeholder {
    class: PHClass::Vec { prio, nonzero },
    name,
  }) = expr.value
  {
    Some((name, prio, nonzero))
  } else {
    None
  }
}
