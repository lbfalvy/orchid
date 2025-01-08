use std::borrow::Borrow;
use std::ops::Deref;
use std::sync::Arc;

pub enum ArcCow<'a, T: ?Sized + ToOwned> {
  Borrowed(&'a T),
  Owned(Arc<T::Owned>),
}
impl<T: ?Sized + ToOwned> ArcCow<'_, T> {
  pub fn owned(value: T::Owned) -> Self { Self::Owned(Arc::new(value)) }
}
impl<T: ?Sized + ToOwned> Clone for ArcCow<'_, T> {
  fn clone(&self) -> Self {
    match self {
      Self::Borrowed(r) => Self::Borrowed(r),
      Self::Owned(b) => Self::Owned(b.clone()),
    }
  }
}

impl<T: ?Sized + ToOwned> Deref for ArcCow<'_, T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    match self {
      Self::Borrowed(t) => t,
      Self::Owned(b) => b.as_ref().borrow(),
    }
  }
}
