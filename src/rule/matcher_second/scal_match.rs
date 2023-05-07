use crate::{rule::state::{State, StateEntry}, ast::{Expr, Clause}};

use super::{shared::ScalMatcher, any_match::any_match};

pub fn scal_match<'a>(matcher: &ScalMatcher, expr: &'a Expr)
-> Option<State<'a>> {
  match (matcher, &expr.value) {
    (ScalMatcher::P(p1), Clause::P(p2)) if p1 == p2 => Some(State::new()),
    (ScalMatcher::Name(n1), Clause::Name(n2)) if n1 == n2
    => Some(State::new()),
    (ScalMatcher::Placeh(key), _)
    => Some(State::from([(*key, StateEntry::Scalar(expr))])),
    (ScalMatcher::S(c1, b_mat), Clause::S(c2, body)) if c1 == c2
    => any_match(b_mat, &body[..]),
    (ScalMatcher::Lambda(arg_mat, b_mat), Clause::Lambda(arg, body)) => {
      let mut state = scal_match(&*arg_mat, &*arg)?;
      state.extend(any_match(&*b_mat, &*body)?);
      Some(state)
    }
    _ => None
  }
}

pub fn scalv_match<'a>(matchers: &[ScalMatcher], seq: &'a [Expr])
-> Option<State<'a>> {
  if seq.len() != matchers.len() {return None}
  let mut state = State::new();
  for (matcher, expr) in matchers.iter().zip(seq.iter()) {
    state.extend(scal_match(matcher, expr)?);
  }
  Some(state)
}