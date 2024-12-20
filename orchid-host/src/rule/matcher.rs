//! Abstract definition of a rule matcher, so that the implementation can
//! eventually be swapped out for a different one.

use std::rc::Rc;

use orchid_base::name::Sym;

use super::state::State;
use crate::macros::MacTree;

/// Cacheable optimized structures for matching patterns on slices. This is
/// injected to allow experimentation in the matcher implementation.
pub trait Matcher {
  /// Build matcher for a pattern
  #[must_use]
  fn new(pattern: Rc<Vec<MacTree>>) -> Self;
  /// Apply matcher to a token sequence
  #[must_use]
  fn apply<'a>(&self, source: &'a [MacTree], save_loc: &impl Fn(Sym) -> bool)
  -> Option<State<'a>>;
}
