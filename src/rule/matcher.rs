use std::rc::Rc;

use super::state::State;
use crate::ast::Expr;
use crate::Sym;

pub type RuleExpr = Expr<Sym>;

/// Cacheable optimized structures for matching patterns on slices. This is
/// injected to allow experimentation in the matcher implementation.
pub trait Matcher {
  /// Build matcher for a pattern
  fn new(pattern: Rc<Vec<RuleExpr>>) -> Self;
  /// Apply matcher to a token sequence
  fn apply<'a>(&self, source: &'a [RuleExpr]) -> Option<State<'a>>;
}
