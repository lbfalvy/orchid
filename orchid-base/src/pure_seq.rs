//! Methods to operate on Rust vectors in a declarative manner

use std::iter;

/// Pure version of [Vec::push]
///
/// Create a new vector consisting of the provided vector with the
/// element appended. See [pushed_ref] to use it with a slice
pub fn pushed<I: IntoIterator, C: FromIterator<I::Item>>(vec: I, t: I::Item) -> C {
  vec.into_iter().chain(iter::once(t)).collect()
}

/// Pure version of [Vec::push]
///
/// Create a new vector consisting of the provided slice with the
/// element appended. See [pushed] for the owned version
pub fn pushed_ref<'a, T: Clone + 'a, C: FromIterator<T>>(
  vec: impl IntoIterator<Item = &'a T>,
  t: T,
) -> C {
  vec.into_iter().cloned().chain(iter::once(t)).collect()
}

/// Push an element on the adhoc stack, pass it to the callback, then pop the
/// element out again.
pub fn with_pushed<T, U>(
  vec: &mut Vec<T>,
  item: T,
  cb: impl for<'a> FnOnce(&'a mut Vec<T>) -> U,
) -> (T, U) {
  vec.push(item);
  let out = cb(vec);
  let item = vec.pop().expect("top element stolen by callback");
  (item, out)
}
