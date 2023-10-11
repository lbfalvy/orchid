use super::any_match::any_match;
use super::shared::ScalMatcher;
use crate::ast::Clause;
use crate::rule::matcher::RuleExpr;
use crate::rule::state::{State, StateEntry};

#[must_use]
pub fn scal_match<'a>(
  matcher: &ScalMatcher,
  expr: &'a RuleExpr,
) -> Option<State<'a>> {
  match (matcher, &expr.value) {
    (ScalMatcher::Atom(a1), Clause::Atom(a2)) if a1.0.strict_eq(&a2.0) =>
      Some(State::new()),
    (ScalMatcher::Name(n1), Clause::Name(n2)) if n1 == n2 => Some(State::new()),
    (ScalMatcher::Placeh(key), _) =>
      Some(State::from([(key.clone(), StateEntry::Scalar(expr))])),
    (ScalMatcher::S(c1, b_mat), Clause::S(c2, body)) if c1 == c2 =>
      any_match(b_mat, &body[..]),
    (ScalMatcher::Lambda(arg_mat, b_mat), Clause::Lambda(arg, body)) => {
      let mut state = any_match(arg_mat, arg)?;
      state.extend(any_match(b_mat, body)?);
      Some(state)
    },
    _ => None,
  }
}

#[must_use]
pub fn scalv_match<'a>(
  matchers: &[ScalMatcher],
  seq: &'a [RuleExpr],
) -> Option<State<'a>> {
  if seq.len() != matchers.len() {
    return None;
  }
  let mut state = State::new();
  for (matcher, expr) in matchers.iter().zip(seq.iter()) {
    state.extend(scal_match(matcher, expr)?);
  }
  Some(state)
}
