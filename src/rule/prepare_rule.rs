use std::rc::Rc;

use hashbrown::HashMap;
use itertools::Itertools;

use crate::representations::location::Location;
use crate::interner::{Token, Interner};
use crate::ast::{PHClass, Expr, Clause, Placeholder, Rule};

use super::RuleError;
use super::vec_attrs::vec_attrs;

/// Ensure that the rule's source begins and ends with a vectorial without
/// changing its meaning
fn pad(mut rule: Rule, i: &Interner) -> Rule {
  let class: PHClass = PHClass::Vec { nonzero: false, prio: 0 };
  let empty_exprv: &[Expr] = &[];
  let prefix_exprv: &[Expr] = &[Expr{
    location: Location::Unknown,
    value: Clause::Placeh(Placeholder{
      name: i.i("::prefix"),
      class
    }),
  }];
  let suffix_exprv: &[Expr] = &[Expr{
    location: Location::Unknown,
    value: Clause::Placeh(Placeholder{
      name: i.i("::suffix"),
      class
    }),
  }];
  let rule_head = rule.source.first().expect("Src can never be empty!");
  let head_explicit = vec_attrs(rule_head).is_some();
  let rule_tail = rule.source.last().expect("Unreachable branch!");
  let tail_explicit = vec_attrs(rule_tail).is_some();
  let prefix_vec = if head_explicit {empty_exprv} else {prefix_exprv};
  let suffix_vec = if tail_explicit {empty_exprv} else {suffix_exprv};
  rule.source = Rc::new(
    prefix_vec.iter()
      .chain(rule.source.iter())
      .chain(suffix_vec.iter())
      .cloned()
      .collect()
  );
  rule.target = Rc::new(
    prefix_vec.iter()
      .chain(rule.target.iter())
      .chain(suffix_vec.iter())
      .cloned()
      .collect()
  );
  rule
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum PHType {
  Scalar,
  Vec{ nonzero: bool }
}
impl From<PHClass> for PHType {
  fn from(value: PHClass) -> Self {
    match value {
      PHClass::Scalar => Self::Scalar,
      PHClass::Vec { nonzero, .. } => Self::Vec{ nonzero }
    }
  }
}

fn check_rec_expr(
  expr: &Expr,
  types: &mut HashMap<Token<String>, PHType>,
  in_template: bool
) -> Result<(), RuleError> {
  match &expr.value {
    Clause::Name(_) | Clause::P(_) => Ok(()),
    Clause::Placeh(Placeholder{ name, class }) => {
      let typ = class.clone().into();
      // in a template, the type must be known and identical
      // outside template (in pattern) the type must be unknown
      if let Some(known) = types.insert(*name, typ) {
        if !in_template { Err(RuleError::Multiple(*name)) }
        else if known != typ { Err(RuleError::TypeMismatch(*name)) }
        else { Ok(()) }
      } else if in_template { Err(RuleError::Missing(*name)) }
      else { Ok(()) }
    }
    Clause::Lambda(arg, body) => {
      check_rec_expr(arg.as_ref(), types, in_template)?;
      check_rec_exprv(&body, types, in_template)
    }
    Clause::S(_, body) => check_rec_exprv(&body, types, in_template)
  }
}

fn check_rec_exprv(
  exprv: &[Expr],
  types: &mut HashMap<Token<String>, PHType>,
  in_template: bool
) -> Result<(), RuleError> {
  for (l, r) in exprv.iter().tuple_windows::<(_, _)>() {
    check_rec_expr(l, types, in_template)?;
    if !in_template { // in a pattern vectorials cannot follow each other
      if let (Some(ld), Some(rd)) = (vec_attrs(l), vec_attrs(r)) {
        return Err(RuleError::VecNeighbors(ld.0, rd.0))
      }
    }
  }
  if let Some(e) = exprv.last() {
    check_rec_expr(e, types, in_template)
  } else { Ok(()) }
}

pub fn prepare_rule(rule: Rule, i: &Interner) -> Result<Rule, RuleError> {
  // Dimension check
  let mut types = HashMap::new();
  check_rec_exprv(&rule.source, &mut types, false)?;
  check_rec_exprv(&rule.target, &mut types, true)?;
  // Padding
  Ok(pad(rule, i))
}