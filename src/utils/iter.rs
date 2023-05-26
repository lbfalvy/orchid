/// Utility functions to get rid of tedious explicit casts to
/// BoxedIter
use std::iter;

/// A trait object of [Iterator] to be assigned to variables that may be
/// initialized form multiple iterators of incompatible types
pub type BoxedIter<'a, T> = Box<dyn Iterator<Item = T> + 'a>;
/// A [BoxedIter] of [BoxedIter].
pub type BoxedIterIter<'a, T> = BoxedIter<'a, BoxedIter<'a, T>>;
/// creates a [BoxedIter] of a single element
pub fn box_once<'a, T: 'a>(t: T) -> BoxedIter<'a, T> {
  Box::new(iter::once(t))
}
/// creates an empty [BoxedIter]
pub fn box_empty<'a, T: 'a>() -> BoxedIter<'a, T> {
  Box::new(iter::empty())
}

/// Chain various iterators into a [BoxedIter]
macro_rules! box_chain {
  ($curr:expr) => {
    Box::new($curr) as BoxedIter<_>
  };
  ($curr:expr, $($rest:expr),*) => {
    Box::new($curr$(.chain($rest))*) as $crate::utils::iter::BoxedIter<_>
  };
}

pub(crate) use box_chain;

/// Flatten an iterator of iterators into a boxed iterator of the inner
/// nested values
pub fn box_flatten<
  'a,
  T: 'a,
  I: 'a + Iterator<Item = J>,
  J: 'a + Iterator<Item = T>,
>(
  i: I,
) -> BoxedIter<'a, T> {
  Box::new(i.flatten())
}

/// Convert an iterator into a `Box<dyn Iterator>`
pub fn into_boxed_iter<'a, T: 'a + IntoIterator>(
  t: T,
) -> BoxedIter<'a, <T as IntoIterator>::Item> {
  Box::new(t.into_iter())
}
