use std::fmt::{Debug, Write};
use std::format;
use std::rc::Rc;

use hashbrown::HashSet;
use itertools::Itertools;
use ordered_float::NotNan;

use super::matcher::{Matcher, RuleExpr};
use super::prepare_rule::prepare_rule;
use super::state::apply_exprv;
use super::{update_first_seq, RuleError, VectreeMatcher};
use crate::ast::Rule;
use crate::interner::{InternedDisplay, Interner};
use crate::Sym;

#[derive(Debug)]
pub struct CachedRule<M: Matcher> {
  matcher: M,
  pattern: Vec<RuleExpr>,
  template: Vec<RuleExpr>,
}

impl<M: InternedDisplay + Matcher> InternedDisplay for CachedRule<M> {
  fn fmt_i(
    &self,
    f: &mut std::fmt::Formatter<'_>,
    i: &Interner,
  ) -> std::fmt::Result {
    for item in self.pattern.iter() {
      item.fmt_i(f, i)?;
      f.write_char(' ')?;
    }
    write!(f, "is matched by ")?;
    self.matcher.fmt_i(f, i)
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
  #[allow(unused)]
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
  #[allow(unused)]
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

fn fmt_hex(num: f64) -> String {
  let exponent = (num.log2() / 4_f64).floor();
  let mantissa = num / 16_f64.powf(exponent);
  format!("0x{:x}p{}", mantissa as i64, exponent as i64)
}

impl<M: InternedDisplay + Matcher> InternedDisplay for Repository<M> {
  fn fmt_i(
    &self,
    f: &mut std::fmt::Formatter<'_>,
    i: &Interner,
  ) -> std::fmt::Result {
    writeln!(f, "Repository[")?;
    for (item, deps, p) in self.cache.iter() {
      write!(
        f,
        "  priority: {}\tdependencies: [{}]\n    ",
        fmt_hex(f64::from(*p)),
        deps.iter().map(|t| i.extern_vec(*t).join("::")).join(", ")
      )?;
      item.fmt_i(f, i)?;
      writeln!(f)?;
    }
    write!(f, "]")
  }
}

/// Repository with the default matcher implementation
pub type Repo = Repository<VectreeMatcher>;
