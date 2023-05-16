use hashbrown::HashMap;

use crate::representations::interpreted::ExprInst;
use crate::interner::{Token, Interner};

#[derive(Clone)]
pub struct Context<'a> {
  pub symbols: &'a HashMap<Token<Vec<Token<String>>>, ExprInst>,
  pub interner: &'a Interner,
  pub gas: Option<usize>,
}

impl Context<'_> {
  pub fn is_stuck(&self, res: Option<usize>) -> bool {
    match (res, self.gas) {
      (Some(a), Some(b)) => a == b,
      (None, None) => false,
      (None, Some(_)) => panic!("gas not tracked despite limit"),
      (Some(_), None) => panic!("gas tracked without request"),
    }
  }
}

#[derive(Clone)]
pub struct Return {
  pub state: ExprInst,
  pub gas: Option<usize>,
  pub inert: bool,
}
