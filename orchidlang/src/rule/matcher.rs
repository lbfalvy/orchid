//! Abstract definition of a rule matcher, so that the implementation can
//! eventually be swapped out for a different one.

use std::rc::Rc;

use super::state::State;
use crate::name::Sym;
use crate::parse::parsed::Expr;

/// The same as [Expr], just extracted for flexibility
pub type RuleExpr = Expr;

/// Cacheable optimized structures for matching patterns on slices. This is
/// injected to allow experimentation in the matcher implementation.
pub trait Matcher {
  /// Build matcher for a pattern
  #[must_use]
  fn new(pattern: Rc<Vec<RuleExpr>>) -> Self;
  /// Apply matcher to a token sequence
  #[must_use]
  fn apply<'a>(&self, source: &'a [RuleExpr], save_loc: &impl Fn(Sym) -> bool)
  -> Option<State<'a>>;
}
