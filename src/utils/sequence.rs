//! An alternative to `Iterable` in many languages, a [Fn] that returns an
//! iterator.

use dyn_clone::{clone_box, DynClone};
use trait_set::trait_set;

use super::boxed_iter::BoxedIter;

trait_set! {
  trait Payload<'a, T> = DynClone + Fn() -> BoxedIter<'a, T> + 'a;
}

/// Dynamic iterator building callback. Given how many trait objects this
/// involves, it may actually be slower than C#.
pub struct Sequence<'a, T: 'a>(Box<dyn Payload<'a, T>>);
impl<'a, T: 'a> Sequence<'a, T> {
  /// Construct from a concrete function returning a concrete iterator
  pub fn new<I: IntoIterator<Item = T> + 'a>(
    f: impl Fn() -> I + 'a + Clone,
  ) -> Self {
    Self(Box::new(move || Box::new(f().into_iter())))
  }
  /// Get an iterator from the function
  pub fn iter(&self) -> impl Iterator<Item = T> + '_ { (self.0)() }
}
impl<'a, T: 'a> Clone for Sequence<'a, T> {
  fn clone(&self) -> Self { Self(clone_box(&*self.0)) }
}
