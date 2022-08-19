use std::fmt::Debug;

use mappable_rc::Mrc;

use crate::expression::Expr;

use super::{super::expression::Rule, executor::execute, RuleError};

pub struct Repository(Vec<Rule>);
impl Repository { 
    pub fn new(mut rules: Vec<Rule>) -> Self {
        rules.sort_by_key(|r| r.prio);
        Self(rules)
    }

    pub fn step(&self, mut code: Mrc<[Expr]>) -> Result<Option<Mrc<[Expr]>>, RuleError> {
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

    pub fn long_step(&self, mut code: Mrc<[Expr]>) -> Result<Mrc<[Expr]>, RuleError> {
        while let Some(tmp) = self.step(Mrc::clone(&code))? {
            code = tmp
        }
        Ok(code)
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
