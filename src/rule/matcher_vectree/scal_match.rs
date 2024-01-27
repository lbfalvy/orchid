use super::any_match::any_match;
use super::shared::ScalMatcher;
use crate::name::Sym;
use crate::parse::parsed::Clause;
use crate::rule::matcher::RuleExpr;
use crate::rule::state::{State, StateEntry};

#[must_use]
pub fn scal_match<'a>(
  matcher: &ScalMatcher,
  expr: &'a RuleExpr,
  save_loc: &impl Fn(Sym) -> bool,
) -> Option<State<'a>> {
  match (matcher, &expr.value) {
    (ScalMatcher::Atom(a1), Clause::Atom(a2))
      if a1.run().0.parser_eq(&a2.run().0) =>
      Some(State::default()),
    (ScalMatcher::Name(n1), Clause::Name(n2)) if n1 == n2 =>
      Some(match save_loc(n1.clone()) {
        true => State::from_name(n1.clone(), expr.range.clone()),
        false => State::default(),
      }),
    (ScalMatcher::Placeh { key, name_only: true }, Clause::Name(n)) =>
      Some(State::from_ph(key.clone(), StateEntry::Name(n, &expr.range))),
    (ScalMatcher::Placeh { key, name_only: false }, _) =>
      Some(State::from_ph(key.clone(), StateEntry::Scalar(expr))),
    (ScalMatcher::S(c1, b_mat), Clause::S(c2, body)) if c1 == c2 =>
      any_match(b_mat, &body[..], save_loc),
    (ScalMatcher::Lambda(arg_mat, b_mat), Clause::Lambda(arg, body)) => Some(
      any_match(arg_mat, arg, save_loc)?
        .combine(any_match(b_mat, body, save_loc)?),
    ),
    _ => None,
  }
}

#[must_use]
pub fn scalv_match<'a>(
  matchers: &[ScalMatcher],
  seq: &'a [RuleExpr],
  save_loc: &impl Fn(Sym) -> bool,
) -> Option<State<'a>> {
  if seq.len() != matchers.len() {
    return None;
  }
  let mut state = State::default();
  for (matcher, expr) in matchers.iter().zip(seq.iter()) {
    state = state.combine(scal_match(matcher, expr, save_loc)?);
  }
  Some(state)
}
