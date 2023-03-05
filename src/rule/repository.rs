use std::fmt::Debug;

use mappable_rc::Mrc;

use crate::representations::ast::Expr;

use super::{super::ast::Rule, executor::execute, RuleError};

/// Manages a priority queue of substitution rules and allows to apply them 
pub struct Repository(Vec<Rule>);
impl Repository { 
  pub fn new(mut rules: Vec<Rule>) -> Self {
    rules.sort_by_key(|r| -r.prio);
    Self(rules)
  }

  pub fn step(&self, code: Mrc<[Expr]>) -> Result<Option<Mrc<[Expr]>>, RuleError> {
    for rule in self.0.iter() {
      if let Some(out) = execute(
        Mrc::clone(&rule.source), Mrc::clone(&rule.target),
        Mrc::clone(&code)
      )? {return Ok(Some(out))}
    }
    Ok(None)
  }

  /// Attempt to run each rule in priority order once
  pub fn pass(&self, mut code: Mrc<[Expr]>) -> Result<Option<Mrc<[Expr]>>, RuleError> {
    let mut ran_once = false;
    for rule in self.0.iter() {
      if let Some(tmp) = execute(
        Mrc::clone(&rule.source), Mrc::clone(&rule.target),
        Mrc::clone(&code)
      )? {
        ran_once = true;
        code = tmp;
      }
    }
    Ok(if ran_once {Some(code)} else {None})
  }

  /// Attempt to run each rule in priority order `limit` times. Returns the final
  /// tree and the number of iterations left to the limit.
  pub fn long_step(&self, mut code: Mrc<[Expr]>, mut limit: usize)
  -> Result<(Mrc<[Expr]>, usize), RuleError> {
    while let Some(tmp) = self.pass(Mrc::clone(&code))? {
      if 0 >= limit {break}
      limit -= 1;
      code = tmp
    }
    Ok((code, limit))
  }
}

impl Debug for Repository {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    for rule in self.0.iter() {
      writeln!(f, "{rule:?}")?
    }
    Ok(())
  } 
}
