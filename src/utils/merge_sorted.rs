use std::mem;

// use itertools::Itertools;

/// Merge two sorted iterators into a sorted iterator.
pub fn merge_sorted<T, I, J, F, O>(mut i: I, mut j: J, mut f: F) -> impl Iterator<Item = T>
where
  I: Iterator<Item = T>, J: Iterator<Item = T>,
  F: FnMut(&T) -> O, O: Ord,
{
  let mut i_item: Option<T> = None;
  let mut j_item: Option<T> = None;
  std::iter::from_fn(move || {
    match (&mut i_item, &mut j_item) {
      (&mut None, &mut None) => None,
      (&mut None, j_item @ &mut Some(_)) => Some((j_item, None)),
      (i_item @ &mut Some(_), &mut None) => Some((i_item, i.next())),
      (Some(i_val), Some(j_val)) => Some(
        if f(i_val) < f(j_val) {
          (&mut i_item, i.next())
        } else {
          (&mut j_item, j.next())
        }
      )
    }.and_then(|(dest, value)| mem::replace(dest, value))
  })
}