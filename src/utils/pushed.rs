use std::iter;

/// Pure version of [Vec::push]
///
/// Create a new vector consisting of the provided vector with the
/// element appended
pub fn pushed<T: Clone>(vec: &[T], t: T) -> Vec<T> {
  vec.iter().cloned().chain(iter::once(t)).collect()
}
