use hashbrown::HashMap;

/// A set that automatically assigns a unique ID to every entry.
///
/// # How unique?
///
/// If you inserted a new entry every nanosecond, it would take more than
/// 550_000 years to run out of indices. Realistically Orchid might insert a new
/// entry every 10ms, so these 64-bit indices will probably outlast humanity.
#[derive(Clone, Debug)]
pub struct IdMap<T> {
  next_id: u64,
  data: HashMap<u64, T>,
}
impl<T> IdMap<T> {
  /// Create a new empty set
  pub fn new() -> Self { Self { next_id: 0, data: HashMap::new() } }

  /// Obtain a reference to the underlying map for iteration
  pub fn map(&self) -> &HashMap<u64, T> { &self.data }

  /// Insert an element with a new ID and return the ID
  pub fn insert(&mut self, t: T) -> u64 {
    let id = self.next_id;
    self.next_id += 1;
    (self.data.try_insert(id, t))
      .unwrap_or_else(|_| panic!("IdMap keys should be unique"));
    id
  }

  /// Obtain a reference to the element with the given ID
  pub fn get(&self, id: u64) -> Option<&T> { self.data.get(&id) }

  /// Obtain a mutable reference to the element with the given ID
  pub fn get_mut(&mut self, id: u64) -> Option<&mut T> {
    self.data.get_mut(&id)
  }

  /// Remove the element with the given ID from the set. The ID will not be
  /// reused.
  pub fn remove(&mut self, id: u64) -> Option<T> { self.data.remove(&id) }
}

impl<T> Default for IdMap<T> {
  fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod test {
  use super::IdMap;

  #[test]
  fn basic_test() {
    let mut map = IdMap::new();
    let a = map.insert(1);
    let b = map.insert(2);
    assert_eq!(map.remove(a), Some(1));
    assert_eq!(map.remove(a), None);
    assert_eq!(map.get(b), Some(&2));
  }
}
