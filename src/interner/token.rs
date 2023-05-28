use std::cmp::PartialEq;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;
use std::num::NonZeroU32;

/// A number representing an object of type `T` stored in some interner. It is a
/// logic error to compare tokens obtained from different interners, or to use a
/// token with an interner other than the one that created it, but this is
/// currently not enforced.
pub struct Tok<T> {
  id: NonZeroU32,
  phantom_data: PhantomData<T>,
}
impl<T> Tok<T> {
  /// Wrap an ID number into a token
  pub fn from_id(id: NonZeroU32) -> Self {
    Self { id, phantom_data: PhantomData }
  }
  /// Take the ID number out of a token
  pub fn into_id(self) -> NonZeroU32 {
    self.id
  }
  /// Cast into usize
  pub fn into_usize(self) -> usize {
    let zero: u32 = self.id.into();
    zero as usize
  }
}

impl<T> Debug for Tok<T> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Token({})", self.id)
  }
}

impl<T> Copy for Tok<T> {}
impl<T> Clone for Tok<T> {
  fn clone(&self) -> Self {
    Self { id: self.id, phantom_data: PhantomData }
  }
}

impl<T> Eq for Tok<T> {}
impl<T> PartialEq for Tok<T> {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl<T> Ord for Tok<T> {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.id.cmp(&other.id)
  }
}
impl<T> PartialOrd for Tok<T> {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl<T> Hash for Tok<T> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    state.write_u32(self.id.into())
  }
}
