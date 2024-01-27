use hashbrown::HashMap;
use intern_all::{i, Tok};
use itertools::Itertools;

use super::matcher::RuleExpr;
use super::rule_error::RuleError;
use super::vec_attrs::vec_attrs;
use crate::parse::parsed::{Clause, PHClass, Placeholder};
use crate::pipeline::project::ProjRule;

/// Ensure that the rule's source begins and ends with a vectorial without
/// changing its meaning
#[must_use]
fn pad(rule: ProjRule) -> ProjRule {
  let prefix_name = i("__gen__orchid__rule__prefix");
  let suffix_name = i("__gen__orchid__rule__suffix");
  let class: PHClass = PHClass::Vec { nonzero: false, prio: 0 };
  let ProjRule { comments, pattern, prio, template } = rule;
  let rule_head = pattern.first().expect("Pattern can never be empty!");
  let rule_tail = pattern.last().unwrap();
  let prefix = vec_attrs(rule_head).is_none().then(|| {
    Clause::Placeh(Placeholder { name: prefix_name, class })
      .into_expr(rule_head.range.map_range(|r| r.start..r.start))
  });
  let suffix = vec_attrs(rule_tail).is_none().then(|| {
    Clause::Placeh(Placeholder { name: suffix_name, class })
      .into_expr(rule_tail.range.map_range(|r| r.start..r.start))
  });
  let pattern =
    prefix.iter().cloned().chain(pattern).chain(suffix.clone()).collect();
  let template = prefix.into_iter().chain(template).chain(suffix).collect();
  ProjRule { comments, prio, pattern, template }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum PHType {
  Scalar,
  Name,
  Vec { nonzero: bool },
}
impl From<PHClass> for PHType {
  fn from(value: PHClass) -> Self {
    match value {
      PHClass::Scalar => Self::Scalar,
      PHClass::Vec { nonzero, .. } => Self::Vec { nonzero },
      PHClass::Name => Self::Name,
    }
  }
}

fn check_rec_expr(
  expr: &RuleExpr,
  types: &mut HashMap<Tok<String>, PHType>,
  in_template: bool,
) -> Result<(), RuleError> {
  match &expr.value {
    Clause::Name(_) | Clause::Atom(_) => Ok(()),
    Clause::Placeh(Placeholder { name, class }) => {
      let typ = (*class).into();
      // in a template, the type must be known and identical
      // outside template (in pattern) the type must be unknown
      if let Some(known) = types.insert(name.clone(), typ) {
        if !in_template {
          Err(RuleError::Multiple(name.clone()))
        } else if known != typ {
          Err(RuleError::ArityMismatch(name.clone()))
        } else {
          Ok(())
        }
      } else if in_template {
        Err(RuleError::Missing(name.clone()))
      } else {
        Ok(())
      }
    },
    Clause::Lambda(arg, body) => {
      check_rec_exprv(arg, types, in_template)?;
      check_rec_exprv(body, types, in_template)
    },
    Clause::S(_, body) => check_rec_exprv(body, types, in_template),
  }
}

fn check_rec_exprv(
  exprv: &[RuleExpr],
  types: &mut HashMap<Tok<String>, PHType>,
  in_template: bool,
) -> Result<(), RuleError> {
  for (l, r) in exprv.iter().tuple_windows::<(_, _)>() {
    check_rec_expr(l, types, in_template)?;
    if !in_template {
      // in a pattern vectorials cannot follow each other
      if let (Some(ld), Some(rd)) = (vec_attrs(l), vec_attrs(r)) {
        return Err(RuleError::VecNeighbors(ld.0, rd.0));
      }
    }
  }
  if let Some(e) = exprv.last() {
    check_rec_expr(e, types, in_template)
  } else {
    Ok(())
  }
}

pub fn prepare_rule(rule: ProjRule) -> Result<ProjRule, RuleError> {
  // Dimension check
  let mut types = HashMap::new();
  check_rec_exprv(&rule.pattern, &mut types, false)?;
  check_rec_exprv(&rule.template, &mut types, true)?;
  // Padding
  Ok(pad(rule))
}
