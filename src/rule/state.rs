use std::rc::Rc;

use hashbrown::HashMap;
use intern_all::Tok;
use itertools::{EitherOrBoth, Itertools};

use super::matcher::RuleExpr;
use crate::location::SourceRange;
use crate::name::Sym;
use crate::parse::parsed::{Clause, Expr, PHClass, Placeholder};
use crate::utils::unwrap_or::unwrap_or;

#[derive(Clone, Copy, Debug)]
pub enum StateEntry<'a> {
  Vec(&'a [RuleExpr]),
  Scalar(&'a RuleExpr),
  Name(&'a Sym, &'a SourceRange),
}
#[derive(Clone)]
pub struct State<'a> {
  placeholders: HashMap<Tok<String>, StateEntry<'a>>,
  name_locations: HashMap<Sym, Vec<SourceRange>>,
}
impl<'a> State<'a> {
  pub fn from_ph(key: Tok<String>, entry: StateEntry<'a>) -> Self {
    Self {
      placeholders: HashMap::from([(key, entry)]),
      name_locations: HashMap::new(),
    }
  }
  pub fn combine(self, s: Self) -> Self {
    Self {
      placeholders: self
        .placeholders
        .into_iter()
        .chain(s.placeholders)
        .collect(),
      name_locations: (self.name_locations.into_iter())
        .sorted_unstable_by_key(|(k, _)| k.id())
        .merge_join_by(
          (s.name_locations.into_iter())
            .sorted_unstable_by_key(|(k, _)| k.id()),
          |(k, _), (k2, _)| k.id().cmp(&k2.id()),
        )
        .map(|ent| match ent {
          EitherOrBoth::Left(i) | EitherOrBoth::Right(i) => i,
          EitherOrBoth::Both((k, l), (_, r)) =>
            (k, l.into_iter().chain(r).collect()),
        })
        .collect(),
    }
  }
  pub fn ph_len(&self, key: &Tok<String>) -> Option<usize> {
    match self.placeholders.get(key)? {
      StateEntry::Vec(slc) => Some(slc.len()),
      _ => None,
    }
  }
  pub fn from_name(name: Sym, location: SourceRange) -> Self {
    Self {
      name_locations: HashMap::from([(name, vec![location])]),
      placeholders: HashMap::new(),
    }
  }
}
impl Default for State<'static> {
  fn default() -> Self {
    Self { name_locations: HashMap::new(), placeholders: HashMap::new() }
  }
}

#[must_use]
pub fn apply_exprv(template: &[RuleExpr], state: &State) -> Vec<RuleExpr> {
  template
    .iter()
    .map(|e| apply_expr(e, state))
    .flat_map(Vec::into_iter)
    .collect()
}

#[must_use]
pub fn apply_expr(template: &RuleExpr, state: &State) -> Vec<RuleExpr> {
  let Expr { range, value } = template;
  match value {
    Clause::Atom(_) | Clause::Name(_) => vec![template.clone()],
    Clause::S(c, body) => vec![Expr {
      range: range.clone(),
      value: Clause::S(*c, Rc::new(apply_exprv(body.as_slice(), state))),
    }],
    Clause::Placeh(Placeholder { name, class }) => {
      let value = *unwrap_or!(state.placeholders.get(name);
        panic!("Placeholder does not have a value in state")
      );
      match (class, value) {
        (PHClass::Scalar, StateEntry::Scalar(item)) => vec![item.clone()],
        (PHClass::Vec { .. }, StateEntry::Vec(chunk)) => chunk.to_vec(),
        (PHClass::Name, StateEntry::Name(n, r)) => {
          vec![RuleExpr { value: Clause::Name(n.clone()), range: r.clone() }]
        },
        _ => panic!("Type mismatch between template and state"),
      }
    },
    Clause::Lambda(arg, body) => vec![Expr {
      range: range.clone(),
      value: Clause::Lambda(
        Rc::new(apply_exprv(arg, state)),
        Rc::new(apply_exprv(&body[..], state)),
      ),
    }],
  }
}
