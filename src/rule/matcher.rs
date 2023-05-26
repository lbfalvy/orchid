use std::rc::Rc;

use super::state::State;
use crate::ast::Expr;

/// Cacheable optimized structures for matching patterns on slices. This is
/// injected to allow experimentation in the matcher implementation.
pub trait Matcher {
  /// Build matcher for a pattern
  fn new(pattern: Rc<Vec<Expr>>) -> Self;
  /// Apply matcher to a token sequence
  fn apply<'a>(&self, source: &'a [Expr]) -> Option<State<'a>>;
}
