use std::fmt::Display;

use super::BoxedIter;

/// A primitive for encoding the two sides Left and Right. While booleans
/// are technically usable for this purpose, they're less descriptive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {Left, Right}

impl Display for Side {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Left => write!(f, "Left"),
      Self::Right => write!(f, "Right"),
    }
  }
}

impl Side {
  pub fn opposite(&self) -> Self {
    match self {
      Self::Left => Self::Right, 
      Self::Right => Self::Left
    }
  }
  /// Shorthand for opposite
  pub fn inv(&self) -> Self { self.opposite() }
  /// take N elements from this end of a slice
  pub fn slice<'a, T>(&self, size: usize, slice: &'a [T]) -> &'a [T] {
    match self {
      Side::Left => &slice[..size],
      Side::Right => &slice[slice.len() - size..]
    }
  }
  /// ignore N elements from this end of a slice
  pub fn crop<'a, T>(&self, margin: usize, slice: &'a [T]) -> &'a [T] {
    self.opposite().slice(slice.len() - margin, slice)
  }
  /// ignore N elements from this end and M elements from the other end
  /// of a slice
  pub fn crop_both<'a, T>(&self,
    margin: usize, opposite: usize, slice: &'a [T]
  ) -> &'a [T] {
    self.crop(margin, self.opposite().crop(opposite, slice))
  }
  /// Pick this side from a pair of things
  pub fn pick<T>(&self, pair: (T, T)) -> T {
    match self {
      Side::Left => pair.0,
      Side::Right => pair.1
    }
  }
  /// Make a pair with the first element on this side
  pub fn pair<T>(&self, this: T, opposite: T) -> (T, T) {
    match self {
      Side::Left => (this, opposite),
      Side::Right => (opposite, this)
    }
  }
  /// Produces an increasing sequence on Right, and a decreasing sequence
  /// on Left
  pub fn walk<'a, I: DoubleEndedIterator + 'a>(&self, iter: I)
  -> BoxedIter<'a, I::Item>
  {
    match self {
      Side::Right => Box::new(iter) as BoxedIter<I::Item>,
      Side::Left => Box::new(iter.rev()),
    }
  }
}

#[cfg(test)]
mod test {
  use itertools::Itertools;
  use super::*;

  /// I apparently have a tendency to mix these up so it's best if
  /// the sides are explicitly stated
  #[test]
  fn test_walk() {
    assert_eq!(
      Side::Right.walk(0..4).collect_vec(),
      vec![0, 1, 2, 3],
      "can walk a range"
    );
    assert_eq!(
      Side::Left.walk(0..4).collect_vec(),
      vec![3, 2, 1, 0],
      "can walk a range backwards"
    )
  }
}
