use orchid_base::name::Sym;

use super::scal_match::scalv_match;
use super::shared::AnyMatcher;
use super::state::MatchState;
use super::vec_match::vec_match;
use crate::macros::MacTree;

#[must_use]
pub fn any_match<'a>(
	matcher: &AnyMatcher,
	seq: &'a [MacTree],
	save_loc: &impl Fn(Sym) -> bool,
) -> Option<MatchState<'a>> {
	match matcher {
		AnyMatcher::Scalar(scalv) => scalv_match(scalv, seq, save_loc),
		AnyMatcher::Vec { left, mid, right } => {
			if seq.len() < left.len() + right.len() {
				return None;
			};
			let left_split = left.len();
			let right_split = seq.len() - right.len();
			Some(
				scalv_match(left, &seq[..left_split], save_loc)?
					.combine(scalv_match(right, &seq[right_split..], save_loc)?)
					.combine(vec_match(mid, &seq[left_split..right_split], save_loc)?),
			)
		},
	}
}
