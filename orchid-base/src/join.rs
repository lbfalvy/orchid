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
  let (val, ev) = try_join_maps::<K, V, Never>(left, right, |k, l, r| Ok(merge(k, l, r)));
  if let Some(e) = ev.first() {
    match *e {}
  }
  val
}

/// Combine two hashmaps via a fallible value merger. See also [join_maps]
pub fn try_join_maps<K: Eq + Hash, V, E>(
  left: HashMap<K, V>,
  mut right: HashMap<K, V>,
  mut merge: impl FnMut(&K, V, V) -> Result<V, E>,
) -> (HashMap<K, V>, Vec<E>) {
  let mut mixed = HashMap::with_capacity(left.len() + right.len());
  let mut errors = Vec::new();
  for (key, lval) in left {
    let val = match right.remove(&key) {
      None => lval,
      Some(rval) => match merge(&key, lval, rval) {
        Ok(v) => v,
        Err(e) => {
          errors.push(e);
          continue;
        },
      },
    };
    mixed.insert(key, val);
  }
  mixed.extend(right);
  (mixed, errors)
}
