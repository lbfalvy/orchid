use std::fmt;
use std::ops::RangeInclusive;

use itertools::Itertools;

use crate::api;

pub type CRange = RangeInclusive<char>;

pub trait ICFilter: fmt::Debug {
	fn ranges(&self) -> &[RangeInclusive<char>];
}
impl ICFilter for [RangeInclusive<char>] {
	fn ranges(&self) -> &[RangeInclusive<char>] { self }
}
impl ICFilter for api::CharFilter {
	fn ranges(&self) -> &[RangeInclusive<char>] { &self.0 }
}

fn try_merge_char_ranges(left: CRange, right: CRange) -> Result<CRange, (CRange, CRange)> {
	match *left.end() as u32 + 1 < *right.start() as u32 {
		true => Err((left, right)),
		false => Ok(*left.start()..=*right.end()),
	}
}

/// Process the character ranges to make them adhere to the structural
/// requirements of [CharFilter]
pub fn mk_char_filter(items: impl IntoIterator<Item = CRange>) -> api::CharFilter {
	api::CharFilter(
		(items.into_iter())
			.filter(|r| *r.start() as u32 <= *r.end() as u32)
			.sorted_by_key(|r| *r.start() as u32)
			.coalesce(try_merge_char_ranges)
			.collect_vec(),
	)
}

/// Decide whether a char filter matches a character via binary search
pub fn char_filter_match(cf: &(impl ICFilter + ?Sized), c: char) -> bool {
	match cf.ranges().binary_search_by_key(&c, |l| *l.end()) {
		Ok(_) => true,                             // c is the end of a range
		Err(i) if i == cf.ranges().len() => false, // all ranges end before c
		Err(i) => cf.ranges()[i].contains(&c),     /* c between cf.0[i-1]?.end and cf.0[i].end,
		                                             * check [i] */
	}
}

/// Merge two char filters into a filter that matches if either of the
/// constituents would match.
pub fn char_filter_union(
	l: &(impl ICFilter + ?Sized),
	r: &(impl ICFilter + ?Sized),
) -> api::CharFilter {
	api::CharFilter(
		(l.ranges().iter().merge_by(r.ranges(), |l, r| l.start() <= r.start()))
			.cloned()
			.coalesce(try_merge_char_ranges)
			.collect_vec(),
	)
}
