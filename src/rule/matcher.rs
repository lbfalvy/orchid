use std::rc::Rc;

use crate::ast::Expr;

use super::state::State;

pub trait Matcher {
  fn new(pattern: Rc<Vec<Expr>>) -> Self;
  fn apply<'a>(&self, source: &'a [Expr]) -> Option<State<'a>>;
}