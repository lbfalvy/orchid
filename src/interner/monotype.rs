use std::borrow::Borrow;
use std::hash::{BuildHasher, Hash};
use std::sync::{RwLock, Arc};

use hashbrown::HashMap;

use super::token::Tok;

/// An interner for any type that implements [Borrow]. This is inspired by
/// Lasso but much simpler, in part because not much can be known about the
/// type.
pub struct TypedInterner<T: 'static + Eq + Hash + Clone> {
  tokens: RwLock<HashMap<Arc<T>, Tok<T>>>,
}
impl<T: Eq + Hash + Clone> TypedInterner<T> {
  /// Create a fresh interner instance
  #[must_use]
  pub fn new() -> Arc<Self> {
    Arc::new(Self { tokens: RwLock::new(HashMap::new()) })
  }

  /// Intern an object, returning a token
  #[must_use]
  pub fn i<Q: ?Sized + Eq + Hash + ToOwned<Owned = T>>(
    self: &Arc<Self>,
    q: &Q,
  ) -> Tok<T>
  where
    T: Borrow<Q>,
  {
    let mut tokens = self.tokens.write().unwrap();
    let hash = compute_hash(tokens.hasher(), q);
    let raw_entry = tokens
      .raw_entry_mut()
      .from_hash(hash, |k| <T as Borrow<Q>>::borrow(k) == q);
    let kv = raw_entry.or_insert_with(|| {
      let keyrc = Arc::new(q.to_owned());
      let token = Tok::<T>::new(keyrc.clone(), Arc::downgrade(self));
      (keyrc, token)
    });
    kv.1.clone()
  }
}

/// Helper function to compute hashes outside a hashmap
#[must_use]
fn compute_hash(
  hash_builder: &impl BuildHasher,
  key: &(impl Hash + ?Sized),
) -> u64 {
  use core::hash::Hasher;
  let mut state = hash_builder.build_hasher();
  key.hash(&mut state);
  state.finish()
}
