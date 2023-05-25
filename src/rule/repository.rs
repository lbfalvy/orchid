use std::fmt::{Debug, Write};
use std::format;
use std::rc::Rc;

use hashbrown::HashSet;
use ordered_float::NotNan;

use super::matcher::Matcher;
use super::prepare_rule::prepare_rule;
use super::state::apply_exprv;
use super::{update_first_seq, AnyMatcher, RuleError};
use crate::ast::{Expr, Rule};
use crate::interner::{InternedDisplay, Interner, Sym};
use crate::utils::Substack;

#[derive(Debug)]
pub struct CachedRule<M: Matcher> {
  matcher: M,
  source: Rc<Vec<Expr>>,
  template: Rc<Vec<Expr>>,
}

impl<M: InternedDisplay + Matcher> InternedDisplay for CachedRule<M> {
  fn fmt_i(
    &self,
    f: &mut std::fmt::Formatter<'_>,
    i: &Interner,
  ) -> std::fmt::Result {
    for item in self.source.iter() {
      item.fmt_i(f, i)?;
      f.write_char(' ')?;
    }
    write!(f, "is matched by ")?;
    self.matcher.fmt_i(f, i)
  }
}

/// Manages a priority queue of substitution rules and allows to apply
/// them
pub struct Repository<M: Matcher> {
  cache: Vec<(CachedRule<M>, HashSet<Sym>, NotNan<f64>)>,
}
impl<M: Matcher> Repository<M> {
  pub fn new(
    mut rules: Vec<Rule>,
    i: &Interner,
  ) -> Result<Self, (Rule, RuleError)> {
    rules.sort_by_key(|r| -r.prio);
    let cache = rules
      .into_iter()
      .map(|r| {
        let prio = r.prio;
        let rule = prepare_rule(r.clone(), i).map_err(|e| (r, e))?;
        let mut glossary = HashSet::new();
        for e in rule.source.iter() {
          e.visit_names(Substack::Bottom, &mut |op| {
            glossary.insert(op);
          })
        }
        let matcher = M::new(rule.source.clone());
        let prep =
          CachedRule { matcher, source: rule.source, template: rule.target };
        Ok((prep, glossary, prio))
      })
      .collect::<Result<Vec<_>, _>>()?;
    Ok(Self { cache })
  }

  /// Attempt to run each rule in priority order once
  pub fn step(&self, code: &Expr) -> Option<Expr> {
    let mut glossary = HashSet::new();
    code.visit_names(Substack::Bottom, &mut |op| {
      glossary.insert(op);
    });
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
  pub fn pass(&self, code: &Expr) -> Option<Expr> {
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
    code: &Expr,
    mut limit: usize,
  ) -> Result<(Expr, usize), RuleError> {
    if limit == 0 {
      return Ok((code.clone(), 0));
    }
    if let Some(mut processed) = self.step(code) {
      limit -= 1;
      if limit == 0 {
        return Ok((processed, 0));
      }
      while let Some(out) = self.step(&processed) {
        limit -= 1;
        if limit == 0 {
          return Ok((out, 0));
        }
        processed = out;
      }
      Ok((processed, limit))
    } else {
      Ok((code.clone(), limit))
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
    for (item, _, p) in self.cache.iter() {
      write!(f, "\t{}", fmt_hex(f64::from(*p)))?;
      item.fmt_i(f, i)?;
      writeln!(f)?;
    }
    write!(f, "]")
  }
}

pub type Repo = Repository<AnyMatcher>;
