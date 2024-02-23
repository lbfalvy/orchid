//! Collects, prioritizes and executes rules.

use std::fmt;
use std::rc::Rc;

use hashbrown::HashSet;
use itertools::Itertools;
use ordered_float::NotNan;

use super::matcher::{Matcher, RuleExpr};
use super::matcher_vectree::shared::VectreeMatcher;
use super::prepare_rule::prepare_rule;
use super::state::apply_exprv;
use super::update_first_seq;
use crate::error::Reporter;
use crate::name::Sym;
use crate::parse::numeric::print_nat16;
use crate::pipeline::project::ProjRule;

#[derive(Debug)]
pub(super) struct CachedRule<M: Matcher> {
  matcher: M,
  pattern: Vec<RuleExpr>,
  pat_glossary: HashSet<Sym>,
  template: Vec<RuleExpr>,
  save_location: HashSet<Sym>,
}

impl<M: fmt::Display + Matcher> fmt::Display for CachedRule<M> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let patterns = self.pattern.iter().join(" ");
    write!(
      f,
      "{patterns} is matched by {} and generates {}",
      self.matcher,
      self.template.iter().map(|e| e.to_string()).join(" ")
    )
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
  cache: Vec<(CachedRule<M>, NotNan<f64>)>,
}
impl<M: Matcher> Repository<M> {
  /// Build a new repository to hold the given set of rules
  pub fn new(mut rules: Vec<ProjRule>, reporter: &Reporter) -> Self {
    rules.sort_by_key(|r| -r.prio);
    let cache = rules
      .into_iter()
      .filter_map(|r| {
        let ProjRule { pattern, prio, template, comments: _ } = prepare_rule(r.clone())
          .inspect_err(|e| reporter.report(e.clone().into_project(&r)))
          .ok()?;
        let mut pat_glossary = HashSet::new();
        pat_glossary.extend(pattern.iter().flat_map(|e| e.value.collect_names().into_iter()));
        let mut tpl_glossary = HashSet::new();
        tpl_glossary.extend(template.iter().flat_map(|e| e.value.collect_names().into_iter()));
        let save_location = pat_glossary.intersection(&tpl_glossary).cloned().collect();
        let matcher = M::new(Rc::new(pattern.clone()));
        let prep = CachedRule { matcher, pattern, template, pat_glossary, save_location };
        Some((prep, prio))
      })
      .collect::<Vec<_>>();
    Self { cache }
  }

  /// Attempt to run each rule in priority order once
  #[must_use]
  pub fn step(&self, code: &RuleExpr) -> Option<RuleExpr> {
    let glossary = code.value.collect_names();
    for (rule, _) in self.cache.iter() {
      if !rule.pat_glossary.is_subset(&glossary) {
        continue;
      }
      let product = update_first_seq::expr(code, &mut |exprv| {
        let save_loc = |n| rule.save_location.contains(&n);
        let state = rule.matcher.apply(exprv.as_slice(), &save_loc)?;
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
  pub fn long_step(&self, code: &RuleExpr, mut limit: usize) -> (RuleExpr, usize) {
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

impl<M: fmt::Debug + Matcher> fmt::Debug for Repository<M> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for rule in self.cache.iter() {
      writeln!(f, "{rule:?}")?
    }
    Ok(())
  }
}

impl<M: fmt::Display + Matcher> fmt::Display for Repository<M> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    writeln!(f, "Repository[")?;
    for (rule, p) in self.cache.iter() {
      let prio = print_nat16(*p);
      let deps = rule.pat_glossary.iter().join(", ");
      writeln!(f, "  priority: {prio}\tdependencies: [{deps}]")?;
      writeln!(f, "    {rule}")?;
    }
    write!(f, "]")
  }
}

/// Repository with the default matcher implementation
pub type Repo = Repository<VectreeMatcher>;
