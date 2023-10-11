use std::fmt::{Debug, Display};
use std::rc::Rc;

use hashbrown::HashSet;
use itertools::Itertools;
use ordered_float::NotNan;

use super::matcher::{Matcher, RuleExpr};
use super::prepare_rule::prepare_rule;
use super::state::apply_exprv;
use super::{update_first_seq, RuleError, VectreeMatcher};
use crate::ast::Rule;
use crate::interner::Interner;
use crate::Sym;
use crate::parse::print_nat16;

#[derive(Debug)]
pub struct CachedRule<M: Matcher> {
  matcher: M,
  pattern: Vec<RuleExpr>,
  template: Vec<RuleExpr>,
}

impl<M: Display + Matcher> Display for CachedRule<M> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let patterns = self.pattern.iter().join(" ");
    write!(f, "{patterns} is matched by {}", self.matcher)
  }
}

/// Substitution rule scheduler
///
/// Manages a priority queue of rules and offers functions to apply them. The
/// rules are stored in an optimized structure but the repository is generic
/// over the implementation of this optimized form.
///
/// If you don't know what to put in the generic parameter, use [Repo]
pub struct Repository<M: Matcher> {
  cache: Vec<(CachedRule<M>, HashSet<Sym>, NotNan<f64>)>,
}
impl<M: Matcher> Repository<M> {
  /// Build a new repository to hold the given set of rules
  pub fn new(
    mut rules: Vec<Rule<Sym>>,
    i: &Interner,
  ) -> Result<Self, (Rule<Sym>, RuleError)> {
    rules.sort_by_key(|r| -r.prio);
    let cache = rules
      .into_iter()
      .map(|r| {
        let prio = r.prio;
        let rule = prepare_rule(r.clone(), i).map_err(|e| (r, e))?;
        let mut glossary = HashSet::new();
        for e in rule.pattern.iter() {
          glossary.extend(e.value.collect_names().into_iter());
        }
        let matcher = M::new(Rc::new(rule.pattern.clone()));
        let prep = CachedRule {
          matcher,
          pattern: rule.pattern,
          template: rule.template,
        };
        Ok((prep, glossary, prio))
      })
      .collect::<Result<Vec<_>, _>>()?;
    Ok(Self { cache })
  }

  /// Attempt to run each rule in priority order once
  #[must_use]
  pub fn step(&self, code: &RuleExpr) -> Option<RuleExpr> {
    let glossary = code.value.collect_names();
    for (rule, deps, _) in self.cache.iter() {
      if !deps.is_subset(&glossary) {
        continue;
      }
      let product = update_first_seq::expr(code, &mut |exprv| {
        let state = rule.matcher.apply(exprv.as_slice())?;
        let result = apply_exprv(&rule.template, &state);
        Some(Rc::new(result))
      });
      if let Some(newcode) = product {
        return Some(newcode);
      }
    }
    None
  }

  /// Keep running the matching rule with the highest priority until no
  /// rules match. WARNING: this function might not terminate
  #[must_use]
  pub fn pass(&self, code: &RuleExpr) -> Option<RuleExpr> {
    if let Some(mut processed) = self.step(code) {
      while let Some(out) = self.step(&processed) {
        processed = out
      }
      Some(processed)
    } else {
      None
    }
  }

  /// Attempt to run each rule in priority order `limit` times. Returns
  /// the final tree and the number of iterations left to the limit.
  #[must_use]
  pub fn long_step(
    &self,
    code: &RuleExpr,
    mut limit: usize,
  ) -> (RuleExpr, usize) {
    if limit == 0 {
      return (code.clone(), 0);
    }
    if let Some(mut processed) = self.step(code) {
      limit -= 1;
      if limit == 0 {
        return (processed, 0);
      }
      while let Some(out) = self.step(&processed) {
        limit -= 1;
        if limit == 0 {
          return (out, 0);
        }
        processed = out;
      }
      (processed, limit)
    } else {
      (code.clone(), limit)
    }
  }
}

impl<M: Debug + Matcher> Debug for Repository<M> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    for rule in self.cache.iter() {
      writeln!(f, "{rule:?}")?
    }
    Ok(())
  }
}

impl<M: Display + Matcher> Display for Repository<M> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    writeln!(f, "Repository[")?;
    for (rule, deps, p) in self.cache.iter() {
      let prio = print_nat16(*p);
      let deps = deps.iter().map(|t| t.extern_vec().join("::")).join(", ");
      writeln!(f, "  priority: {prio}\tdependencies: [{deps}]")?;
      writeln!(f, "    {rule}")?;
    }
    write!(f, "]")
  }
}

/// Repository with the default matcher implementation
pub type Repo = Repository<VectreeMatcher>;
