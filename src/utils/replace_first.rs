use std::iter;

/// Iterate over a sequence with the first element updated for which the
/// function returns Some(), but only if there is such an element.
pub fn replace_first<T: Clone, F: FnMut(&T) -> Option<T>>(
  slice: &[T],
  mut f: F,
) -> Option<impl Iterator<Item = T> + '_> {
  for i in 0..slice.len() {
    if let Some(new) = f(&slice[i]) {
      let subbed_iter = slice[0..i]
        .iter()
        .cloned()
        .chain(iter::once(new))
        .chain(slice[i + 1..].iter().cloned());
      return Some(subbed_iter);
    }
  }
  None
}
