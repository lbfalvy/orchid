/// Utility functions to get rid of explicit casts to BoxedIter which are tedious

use std::{iter, mem};

pub type BoxedIter<'a, T> = Box<dyn Iterator<Item = T> + 'a>;
pub type BoxedIterIter<'a, T> = BoxedIter<'a, BoxedIter<'a, T>>;
/// BoxedIter of a single element
pub fn box_once<'a, T: 'a>(t: T) -> BoxedIter<'a, T> {
  Box::new(iter::once(t))
}
/// BoxedIter of no elements
pub fn box_empty<'a, T: 'a>() -> BoxedIter<'a, T> {
  Box::new(iter::empty())
}

#[macro_export]
macro_rules! box_chain {
  ($curr:expr) => {
    Box::new($curr) as BoxedIter<_>
  };
  ($curr:expr, $($rest:expr),*) => {
    Box::new($curr$(.chain($rest))*) as $crate::utils::iter::BoxedIter<_>
  };
}

pub fn box_flatten<'a, T: 'a, I: 'a, J: 'a>(i: I) -> BoxedIter<'a, T>
where
  J: Iterator<Item = T>,
  I: Iterator<Item = J>,
{
  Box::new(i.flatten())
}

pub fn into_boxed_iter<'a, T: 'a>(t: T) -> BoxedIter<'a, <T as IntoIterator>::Item>
where T: IntoIterator {
  Box::new(t.into_iter())
}