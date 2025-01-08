use std::fmt;

use itertools::Itertools;
use orchid_api::PhKind;
use orchid_base::intern;
use orchid_base::location::Pos;
use orchid_base::name::Sym;
use orchid_base::tree::Ph;

use super::any_match::any_match;
use super::build::mk_any;
use super::shared::{AnyMatcher, VecMatcher};
use super::state::{MatchState, StateEntry};
use super::vec_attrs::vec_attrs;
use super::vec_match::vec_match;
use crate::macros::{MacTok, MacTree};
use crate::rule::build::mk_vec;

pub fn first_is_vec(pattern: &[MacTree]) -> bool { vec_attrs(pattern.first().unwrap()).is_some() }
pub fn last_is_vec(pattern: &[MacTree]) -> bool { vec_attrs(pattern.last().unwrap()).is_some() }

pub struct NamedMatcher(AnyMatcher);
impl NamedMatcher {
	pub fn new(pattern: &[MacTree]) -> Self {
		assert!(
			matches!(pattern.first().map(|tree| &*tree.tok), Some(MacTok::Name(_))),
			"Named matchers must begin with a name"
		);

		match last_is_vec(pattern) {
			true => Self(mk_any(pattern)),
			false => {
				let kind: PhKind = PhKind::Vector { priority: 0, at_least_one: false };
				let suffix = [MacTok::Ph(Ph { name: intern!(str: "::after"), kind }).at(Pos::None)];
				Self(mk_any(&pattern.iter().chain(&suffix).cloned().collect_vec()))
			},
		}
	}
	/// Also returns the tail, if any, which should be matched further
	/// Note that due to how priod works below, the main usable information from
	/// the tail is its length
	pub fn apply<'a>(
		&self,
		seq: &'a [MacTree],
		save_loc: impl Fn(Sym) -> bool,
	) -> Option<(MatchState<'a>, &'a [MacTree])> {
		any_match(&self.0, seq, &save_loc).map(|mut state| {
			match state.remove(intern!(str: "::after")) {
				Some(StateEntry::Scalar(_)) => panic!("::after can never be a scalar entry!"),
				Some(StateEntry::Vec(v)) => (state, v),
				None => (state, &[][..]),
			}
		})
	}
}
impl fmt::Display for NamedMatcher {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}
impl fmt::Debug for NamedMatcher {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "NamedMatcher({self})") }
}

pub struct PriodMatcher(VecMatcher);
impl PriodMatcher {
	pub fn new(pattern: &[MacTree]) -> Self {
		assert!(
			pattern.first().and_then(vec_attrs).is_some() && pattern.last().and_then(vec_attrs).is_some(),
			"Prioritized matchers must start and end with a vectorial",
		);
		Self(mk_vec(pattern))
	}
	/// tokens before the offset always match the prefix
	pub fn apply<'a>(
		&self,
		seq: &'a [MacTree],
		save_loc: impl Fn(Sym) -> bool,
	) -> Option<MatchState<'a>> {
		vec_match(&self.0, seq, &save_loc)
	}
}
impl fmt::Display for PriodMatcher {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}
impl fmt::Debug for PriodMatcher {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "PriodMatcher({self})") }
}
