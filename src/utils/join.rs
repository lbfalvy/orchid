//! Join hashmaps with a callback for merging or failing on conflicting keys.

use std::hash::Hash;

use hashbrown::HashMap;
use never::Never;

/// Combine two hashmaps via an infallible value merger. See also
/// [try_join_maps]
pub fn join_maps<K: Eq + Hash, V>(
  left: HashMap<K, V>,
  right: HashMap<K, V>,
  mut merge: impl FnMut(&K, V, V) -> V,
) -> HashMap<K, V> {
  try_join_maps(left, right, |k, l, r| Ok(merge(k, l, r))).unwrap_or_else(|e: Never| match e {})
}

/// Combine two hashmaps via a fallible value merger. See also [join_maps]
pub fn try_join_maps<K: Eq + Hash, V, E>(
  left: HashMap<K, V>,
  mut right: HashMap<K, V>,
  mut merge: impl FnMut(&K, V, V) -> Result<V, E>,
) -> Result<HashMap<K, V>, E> {
  let mut mixed = HashMap::with_capacity(left.len() + right.len());
  for (key, lval) in left {
    let val = match right.remove(&key) {
      None => lval, 
      Some(rval) => merge(&key, lval, rval)?,
    };
    mixed.insert(key, val);
  }
  mixed.extend(right);
  Ok(mixed)
}
