/// Utility functions to get rid of tedious explicit casts to
/// BoxedIter
use std::iter;

/// A trait object of [Iterator] to be assigned to variables that may be
/// initialized form multiple iterators of incompatible types
pub type BoxedIter<'a, T> = Box<dyn Iterator<Item = T> + 'a>;
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
