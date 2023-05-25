use std::rc::Rc;

use super::state::State;
use crate::ast::Expr;

pub trait Matcher {
  fn new(pattern: Rc<Vec<Expr>>) -> Self;
  fn apply<'a>(&self, source: &'a [Expr]) -> Option<State<'a>>;
}
