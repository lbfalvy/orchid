use std::num::NonZeroU32;
use std::cell::RefCell;
use std::borrow::Borrow;
use std::hash::{Hash, BuildHasher};

use hashbrown::HashMap;

use super::token::Token;

pub struct TypedInterner<T: 'static + Eq + Hash + Clone>{
  tokens: RefCell<HashMap<&'static T, Token<T>>>,
  values: RefCell<Vec<(&'static T, bool)>>
}
impl<T: Eq + Hash + Clone> TypedInterner<T> {
  /// Create a fresh interner instance
  pub fn new() -> Self {
    Self {
      tokens: RefCell::new(HashMap::new()),
      values: RefCell::new(Vec::new())
    }
  }

  /// Intern an object, returning a token
  pub fn i<Q: ?Sized + Eq + Hash + ToOwned<Owned = T>>(&self, q: &Q)
  -> Token<T> where T: Borrow<Q>
  {
    let mut tokens = self.tokens.borrow_mut();
    let hash = compute_hash(tokens.hasher(), q);
    let raw_entry = tokens.raw_entry_mut().from_hash(hash, |k| {
      <T as Borrow<Q>>::borrow(k) == q
    });
    let kv = raw_entry.or_insert_with(|| {
      let mut values = self.values.borrow_mut();
      let uniq_key: NonZeroU32 = (values.len() as u32 + 1u32)
        .try_into().expect("can never be zero");
      let keybox = Box::new(q.to_owned());
      let keyref = Box::leak(keybox);
      values.push((keyref, true));
      let token = Token::<T>::from_id(uniq_key);
      (keyref, token)
    });
    *kv.1
  }

  /// Resolve a token, obtaining an object
  /// It is illegal to use a token obtained from one interner with another.
  pub fn r(&self, t: Token<T>) -> &T {
    let values = self.values.borrow();
    let key = t.into_usize() - 1;
    values[key].0
  }

  /// Intern a static reference without allocating the data on the heap
  #[allow(unused)]
  pub fn intern_static(&self, tref: &'static T) -> Token<T> {
    let mut tokens = self.tokens.borrow_mut();
    let token = *tokens.raw_entry_mut().from_key(tref)
    .or_insert_with(|| {
      let mut values = self.values.borrow_mut();
      let uniq_key: NonZeroU32 = (values.len() as u32 + 1u32)
        .try_into().expect("can never be zero");
      values.push((tref, false));
      let token = Token::<T>::from_id(uniq_key);
      (tref, token)
    }).1;
    token
  }
}

impl<T: Eq + Hash + Clone> Drop for TypedInterner<T> {
  fn drop(&mut self) {
    // make sure all values leaked by us are dropped
    // FIXME: with the new hashmap logic we can actually store Rc-s
    // which negates the need for unsafe here
    let mut values = self.values.borrow_mut();
    for (item, owned) in values.drain(..) {
      if !owned {continue}
      unsafe {
        Box::from_raw((item as *const T).cast_mut())
      };
    }
  }
}

/// Helper function to compute hashes outside a hashmap
fn compute_hash(
  hash_builder: &impl BuildHasher,
  key: &(impl Hash + ?Sized)
) -> u64 {
  use core::hash::Hasher;
  let mut state = hash_builder.build_hasher();
  key.hash(&mut state);
  state.finish()
}