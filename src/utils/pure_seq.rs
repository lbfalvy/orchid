use std::iter;

/// Pure version of [Vec::push]
///
/// Create a new vector consisting of the provided vector with the
/// element appended. See [pushed_ref] to use it with a slice
pub fn pushed<T: Clone>(vec: impl IntoIterator<Item = T>, t: T) -> Vec<T> {
  vec.into_iter().chain(iter::once(t)).collect()
}

/// Pure version of [Vec::push]
///
/// Create a new vector consisting of the provided slice with the
/// element appended. See [pushed] for the owned version
pub fn pushed_ref<'a, T: Clone + 'a>(
  vec: impl IntoIterator<Item = &'a T>,
  t: T,
) -> Vec<T> {
  vec.into_iter().cloned().chain(iter::once(t)).collect()
}

/// Pure version of [Iterator::next]
///
/// Remove an item from the iterator. If successful, returns the item and the
/// iterator. If the iterator is empty it is consumed.
pub fn next<I: Iterator>(mut i: I) -> Option<(I::Item, I)> {
  i.next().map(|item| (item, i))
}
