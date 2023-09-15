use std::cmp::PartialEq;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::num::NonZeroUsize;
use std::ops::Deref;
use std::rc::{Rc, Weak};

use super::TypedInterner;

/// A number representing an object of type `T` stored in some interner. It is a
/// logic error to compare tokens obtained from different interners, or to use a
/// token with an interner other than the one that created it, but this is
/// currently not enforced.
#[derive(Clone)]
pub struct Tok<T: Eq + Hash + Clone + 'static> {
  data: Rc<T>,
  interner: Weak<TypedInterner<T>>,
}
impl<T: Eq + Hash + Clone + 'static> Tok<T> {
  /// Create a new token. Used exclusively by the interner
  pub(crate) fn new(data: Rc<T>, interner: Weak<TypedInterner<T>>) -> Self {
    Self { data, interner }
  }
  /// Take the ID number out of a token
  pub fn id(&self) -> NonZeroUsize {
    ((self.data.as_ref() as *const T as usize).try_into())
      .expect("Pointer can always be cast to nonzero")
  }
  /// Cast into usize
  pub fn usize(&self) -> usize { self.id().into() }
  ///
  pub fn assert_comparable(&self, other: &Self) {
    let iref = self.interner.as_ptr() as usize;
    assert!(
      iref == other.interner.as_ptr() as usize,
      "Tokens must come from the same interner"
    );
  }
}

impl<T: Eq + Hash + Clone + 'static> Tok<Vec<Tok<T>>> {
  /// Extern all elements of the vector in a new vector
  pub fn extern_vec(&self) -> Vec<T> {
    self.iter().map(|t| (**t).clone()).collect()
  }
}

impl<T: Eq + Hash + Clone + 'static> Deref for Tok<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target { self.data.as_ref() }
}

impl<T: Eq + Hash + Clone + 'static + Debug> Debug for Tok<T> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Token({} -> {:?})", self.id(), self.data.as_ref())
  }
}

impl<T: Eq + Hash + Clone + Display + 'static> Display for Tok<T> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", **self)
  }
}

impl<T: Eq + Hash + Clone + 'static> Eq for Tok<T> {}
impl<T: Eq + Hash + Clone + 'static> PartialEq for Tok<T> {
  fn eq(&self, other: &Self) -> bool {
    self.assert_comparable(other);
    self.id() == other.id()
  }
}

impl<T: Eq + Hash + Clone + 'static> Ord for Tok<T> {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.assert_comparable(other);
    self.id().cmp(&other.id())
  }
}
impl<T: Eq + Hash + Clone + 'static> PartialOrd for Tok<T> {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl<T: Eq + Hash + Clone + 'static> Hash for Tok<T> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    state.write_usize(self.usize())
  }
}
