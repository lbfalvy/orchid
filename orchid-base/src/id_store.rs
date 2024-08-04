use std::num::NonZeroU64;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use hashbrown::HashMap;

pub struct IdStore<T> {
  table: OnceLock<Mutex<HashMap<NonZeroU64, T>>>,
  id: AtomicU64,
}
impl<T> IdStore<T> {
  pub const fn new() -> Self { Self { table: OnceLock::new(), id: AtomicU64::new(1) } }
  pub fn add(&self, t: T) -> IdRecord<'_, T> {
    let tbl = self.table.get_or_init(Mutex::default);
    let mut tbl_g = tbl.lock().unwrap();
    let id: NonZeroU64 = self.id.fetch_add(1, Ordering::Relaxed).try_into().unwrap();
    assert!(tbl_g.insert(id, t).is_none(), "atom ID wraparound");
    IdRecord(id, tbl_g)
  }
  pub fn get(&self, id: impl Into<NonZeroU64>) -> Option<IdRecord<'_, T>> {
    let tbl = self.table.get_or_init(Mutex::default);
    let tbl_g = tbl.lock().unwrap();
    let id64 = id.into();
    if tbl_g.contains_key(&id64) { Some(IdRecord(id64, tbl_g)) } else { None }
  }
  pub fn is_empty(&self) -> bool { self.len() == 0 }
  pub fn len(&self) -> usize {
    self.table.get().map(|t| t.lock().unwrap().len()).unwrap_or(0)
  }
}

impl<T> Default for IdStore<T> {
  fn default() -> Self { Self::new() }
}

pub struct IdRecord<'a, T>(NonZeroU64, MutexGuard<'a, HashMap<NonZeroU64, T>>);
impl<'a, T> IdRecord<'a, T> {
  pub fn id(&self) -> NonZeroU64 { self.0 }
  pub fn remove(mut self) -> T { self.1.remove(&self.0).unwrap() }
}
impl<'a, T> Deref for IdRecord<'a, T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    self.1.get(&self.0).expect("Existence checked on construction")
  }
}
impl<'a, T> DerefMut for IdRecord<'a, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.1.get_mut(&self.0).expect("Existence checked on construction")
  }
}
