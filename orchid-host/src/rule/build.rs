use itertools::Itertools;
use orchid_api::PhKind;
use orchid_base::interner::Tok;
use orchid_base::side::Side;
use orchid_base::tree::Ph;

use super::shared::{AnyMatcher, ScalMatcher, VecMatcher};
use crate::macros::{MacTok, MacTree};
use crate::rule::vec_attrs::vec_attrs;

pub type MaxVecSplit<'a> = (&'a [MacTree], (Tok<String>, u8, bool), &'a [MacTree]);

/// Derive the details of the central vectorial and the two sides from a
/// slice of Expr's
#[must_use]
fn split_at_max_vec(pattern: &[MacTree]) -> Option<MaxVecSplit> {
	let rngidx = pattern
		.iter()
		.position_max_by_key(|expr| vec_attrs(expr).map(|attrs| attrs.1 as i64).unwrap_or(-1))?;
	let (left, not_left) = pattern.split_at(rngidx);
	let (placeh, right) =
		not_left.split_first().expect("The index of the greatest element must be less than the length");
	vec_attrs(placeh).map(|attrs| (left, attrs, right))
}

#[must_use]
fn scal_cnt<'a>(iter: impl Iterator<Item = &'a MacTree>) -> usize {
	iter.take_while(|expr| vec_attrs(expr).is_none()).count()
}

#[must_use]
pub fn mk_any(pattern: &[MacTree]) -> AnyMatcher {
	let left_split = scal_cnt(pattern.iter());
	if pattern.len() <= left_split {
		return AnyMatcher::Scalar(mk_scalv(pattern));
	}
	let (left, not_left) = pattern.split_at(left_split);
	let right_split = not_left.len() - scal_cnt(pattern.iter().rev());
	let (mid, right) = not_left.split_at(right_split);
	AnyMatcher::Vec { left: mk_scalv(left), mid: mk_vec(mid), right: mk_scalv(right) }
}

/// Pattern MUST NOT contain vectorial placeholders
#[must_use]
fn mk_scalv(pattern: &[MacTree]) -> Vec<ScalMatcher> { pattern.iter().map(mk_scalar).collect() }

/// Pattern MUST start and end with a vectorial placeholder
#[must_use]
pub fn mk_vec(pattern: &[MacTree]) -> VecMatcher {
	debug_assert!(!pattern.is_empty(), "pattern cannot be empty");
	debug_assert!(pattern.first().map(vec_attrs).is_some(), "pattern must start with a vectorial");
	debug_assert!(pattern.last().map(vec_attrs).is_some(), "pattern must end with a vectorial");
	let (left, (key, _, nonzero), right) = split_at_max_vec(pattern)
		.expect("pattern must have vectorial placeholders at least at either end");
	let r_sep_size = scal_cnt(right.iter());
	let (r_sep, r_side) = right.split_at(r_sep_size);
	let l_sep_size = scal_cnt(left.iter().rev());
	let (l_side, l_sep) = left.split_at(left.len() - l_sep_size);
	let main = VecMatcher::Placeh { key: key.clone(), nonzero };
	match (left, right) {
		(&[], &[]) => VecMatcher::Placeh { key, nonzero },
		(&[], _) => VecMatcher::Scan {
			direction: Side::Left,
			left: Box::new(main),
			sep: mk_scalv(r_sep),
			right: Box::new(mk_vec(r_side)),
		},
		(_, &[]) => VecMatcher::Scan {
			direction: Side::Right,
			left: Box::new(mk_vec(l_side)),
			sep: mk_scalv(l_sep),
			right: Box::new(main),
		},
		(..) => {
			let mut key_order =
				l_side.iter().chain(r_side.iter()).filter_map(vec_attrs).collect::<Vec<_>>();
			key_order.sort_by_key(|(_, prio, _)| -(*prio as i64));
			VecMatcher::Middle {
				left: Box::new(mk_vec(l_side)),
				left_sep: mk_scalv(l_sep),
				mid: Box::new(main),
				right_sep: mk_scalv(r_sep),
				right: Box::new(mk_vec(r_side)),
				key_order: key_order.into_iter().map(|(n, ..)| n).collect(),
			}
		},
	}
}

/// Pattern MUST NOT be a vectorial placeholder
#[must_use]
fn mk_scalar(pattern: &MacTree) -> ScalMatcher {
	match &*pattern.tok {
		MacTok::Atom(_) | MacTok::Done(_) => panic!("Atoms and Done aren't supported in matchers"),
		MacTok::Name(n) => ScalMatcher::Name(n.clone()),
		MacTok::Ph(Ph { name, kind }) => match kind {
			PhKind::Vector { .. } => {
				panic!("Scalar matcher cannot be built from vector pattern")
			},
			PhKind::Scalar => ScalMatcher::Placeh { key: name.clone() },
		},
		MacTok::S(c, body) => ScalMatcher::S(*c, Box::new(mk_any(body))),
		MacTok::Lambda(arg, body) => ScalMatcher::Lambda(Box::new(mk_any(arg)), Box::new(mk_any(body))),
		MacTok::Ref(_) | MacTok::Slot(_) => panic!("Extension-only variants"),
	}
}

#[cfg(test)]
mod test {
	use std::sync::Arc;

	use orchid_api::PhKind;
	use orchid_base::location::SourceRange;
	use orchid_base::tokens::Paren;
	use orchid_base::tree::Ph;
	use orchid_base::{intern, sym};

	use super::mk_any;
	use crate::macros::{MacTok, MacTree};

	#[test]
	fn test_scan() {
		let ex = |tok: MacTok| MacTree { tok: Arc::new(tok), pos: SourceRange::mock().pos() };
		let pattern = vec![
			ex(MacTok::Ph(Ph {
				kind: PhKind::Vector { priority: 0, at_least_one: false },
				name: intern!(str: "::prefix"),
			})),
			ex(MacTok::Name(sym!(prelude::do))),
			ex(MacTok::S(Paren::Round, vec![
				ex(MacTok::Ph(Ph {
					kind: PhKind::Vector { priority: 0, at_least_one: false },
					name: intern!(str: "expr"),
				})),
				ex(MacTok::Name(sym!(prelude::;))),
				ex(MacTok::Ph(Ph {
					kind: PhKind::Vector { priority: 1, at_least_one: false },
					name: intern!(str: "rest"),
				})),
			])),
			ex(MacTok::Ph(Ph {
				kind: PhKind::Vector { priority: 0, at_least_one: false },
				name: intern!(str: "::suffix"),
			})),
		];
		let matcher = mk_any(&pattern);
		println!("{matcher}");
	}
}
