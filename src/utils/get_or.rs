use std::hash::Hash;

use hashbrown::HashMap;

/// Get the given value from the map or initialize it with the callback if it
/// doesn't exist, then return a mutable reference.
pub fn get_or_make<'a, K: Eq + Hash + Clone, V>(
  map: &'a mut HashMap<K, V>,
  k: &K,
  make: impl FnOnce() -> V,
) -> &'a mut V {
  map.raw_entry_mut().from_key(k).or_insert_with(|| (k.clone(), make())).1
}
