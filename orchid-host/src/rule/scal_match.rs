use orchid_base::name::Sym;

use super::any_match::any_match;
use super::shared::ScalMatcher;
use crate::macros::{MacTok, MacTree};
use crate::rule::state::{MatchState, StateEntry};

#[must_use]
pub fn scal_match<'a>(
	matcher: &ScalMatcher,
	expr: &'a MacTree,
	save_loc: &impl Fn(Sym) -> bool,
) -> Option<MatchState<'a>> {
	match (matcher, &*expr.tok) {
		(ScalMatcher::Name(n1), MacTok::Name(n2)) if n1 == n2 => Some(match save_loc(n1.clone()) {
			true => MatchState::from_name(n1.clone(), expr.pos.clone()),
			false => MatchState::default(),
		}),
		(ScalMatcher::Placeh { .. }, MacTok::Done(_)) => None,
		(ScalMatcher::Placeh { key }, _) =>
			Some(MatchState::from_ph(key.clone(), StateEntry::Scalar(expr))),
		(ScalMatcher::S(c1, b_mat), MacTok::S(c2, body)) if c1 == c2 =>
			any_match(b_mat, &body[..], save_loc),
		(ScalMatcher::Lambda(arg_mat, b_mat), MacTok::Lambda(arg, body)) =>
			Some(any_match(arg_mat, arg, save_loc)?.combine(any_match(b_mat, body, save_loc)?)),
		_ => None,
	}
}

#[must_use]
pub fn scalv_match<'a>(
	matchers: &[ScalMatcher],
	seq: &'a [MacTree],
	save_loc: &impl Fn(Sym) -> bool,
) -> Option<MatchState<'a>> {
	if seq.len() != matchers.len() {
		return None;
	}
	let mut state = MatchState::default();
	for (matcher, expr) in matchers.iter().zip(seq.iter()) {
		state = state.combine(scal_match(matcher, expr, save_loc)?);
	}
	Some(state)
}
