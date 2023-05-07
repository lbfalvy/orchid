use std::{num::NonZeroU32, marker::PhantomData};
use std::fmt::Debug;
use std::hash::Hash;

use std::cmp::PartialEq;

pub struct Token<T>{
  id: NonZeroU32,
  phantom_data: PhantomData<T>
}
impl<T> Token<T> {
  pub fn from_id(id: NonZeroU32) -> Self {
    Self { id, phantom_data: PhantomData }
  }
  pub fn into_id(self) -> NonZeroU32 {
    self.id
  }
  pub fn into_usize(self) -> usize {
    let zero: u32 = self.id.into();
    zero as usize
  }
}

impl<T> Debug for Token<T> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Token({})", self.id)
  }
}

impl<T> Copy for Token<T> {}
impl<T> Clone for Token<T> {
  fn clone(&self) -> Self {
    Self{ id: self.id, phantom_data: PhantomData }
  }
}

impl<T> Eq for Token<T> {}
impl<T> PartialEq for Token<T> {
  fn eq(&self, other: &Self) -> bool { self.id == other.id }
}

impl<T> Ord for Token<T> {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.id.cmp(&other.id)
  }
}
impl<T> PartialOrd for Token<T> {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(&other))
  }
}

impl<T> Hash for Token<T> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    state.write_u32(self.id.into())
  }
}