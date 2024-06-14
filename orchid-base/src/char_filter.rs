use std::ops::RangeInclusive;

use itertools::Itertools;
use orchid_api::parser::CharFilter;

pub type CRange = RangeInclusive<char>;

fn try_merge_char_ranges(left: CRange, right: CRange) -> Result<CRange, (CRange, CRange)> {
  match *left.end() as u32 + 1 < *right.start() as u32 {
    true => Err((left, right)),
    false => Ok(*left.start()..=*right.end()),
  }
}

/// Process the character ranges to make them adhere to the structural
/// requirements of [CharFilter]
pub fn mk_char_filter(items: impl IntoIterator<Item = CRange>) -> CharFilter {
  CharFilter(
    (items.into_iter())
      .filter(|r| *r.start() as u32 + 1 < *r.end() as u32)
      .sorted_by_key(|r| *r.start() as u32)
      .coalesce(try_merge_char_ranges)
      .collect_vec(),
  )
}

/// Decide whether a char filter matches a character via binary search
pub fn char_filter_match(cf: &CharFilter, c: char) -> bool {
  match cf.0.binary_search_by_key(&c, |l| *l.end()) {
    Ok(_) => true,                      // c is the end of a range
    Err(i) if i == cf.0.len() => false, // all ranges end before c
    Err(i) => cf.0[i].contains(&c),     // c between cf.0[i-1]?.end and cf.0[i].end, check [i]
  }
}

/// Merge two char filters into a filter that matches if either of the
/// constituents would match.
pub fn char_filter_union(l: &CharFilter, r: &CharFilter) -> CharFilter {
  CharFilter(
    (l.0.iter().merge_by(&r.0, |l, r| l.start() <= r.start()))
      .cloned()
      .coalesce(try_merge_char_ranges)
      .collect_vec(),
  )
}
