use std::hash::Hash;

use hashbrown::HashMap;

/// Return the given value from the map or default-initialize if it doesn't
/// exist, then retunrn a mutable reference.
pub fn get_or_default<'a, K: Eq + Hash + Clone, V: Default>(
  map: &'a mut HashMap<K, V>,
  k: &K,
) -> &'a mut V {
  get_or_make(map, k, || V::default())
}

pub fn get_or_make<'a, K: Eq + Hash + Clone, V>(
  map: &'a mut HashMap<K, V>,
  k: &K,
  make: impl FnOnce() -> V,
) -> &'a mut V {
  map.raw_entry_mut().from_key(k).or_insert_with(|| (k.clone(), make())).1
}
