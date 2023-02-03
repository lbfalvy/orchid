use std::{iter, ops::{Index, Add}, borrow::Borrow};

use smallvec::SmallVec;

const INLINE_ENTRIES: usize = 2;

/// Linked-array-list of key-value pairs.
/// Lookup and modification is O(n + cachemiss * n / m)
/// Can be extended by reference in O(m) < O(n)
/// 
/// The number of elements stored inline in a stackframe is 2 by default, which is enough for most
/// recursive algorithms. The cost of overruns is a heap allocation and subsequent heap indirections,
/// plus wasted stack space which is likely wasted L1 as well. The cost of underruns is wasted stack
/// space.
pub struct ProtoMap<'a, K, V, const STACK_COUNT: usize = 2> {
  entries: SmallVec<[(K, Option<V>); STACK_COUNT]>,
  prototype: Option<&'a ProtoMap<'a, K, V, STACK_COUNT>>
}

impl<'a, K, V, const STACK_COUNT: usize> ProtoMap<'a, K, V, STACK_COUNT> {
  pub fn new() -> Self {
    Self {
      entries: SmallVec::new(),
      prototype: None
    }
  }

  /// Mutable reference to entry without checking proto in O(m)
  fn local_entry_mut<'b, Q: ?Sized>(&'b mut self, query: &Q)
  -> Option<(usize, &'b mut K, &'b mut Option<V>)>
  where K: Borrow<Q>, Q: Eq
  {
    self.entries.iter_mut().enumerate().find_map(|(i, (k, v))| {
      if query.eq((*k).borrow()) { Some((i, k, v)) } else { None }
    })
  }

  /// Entry without checking proto in O(m)
  fn local_entry<'b, Q: ?Sized>(&'b self, query: &Q)
  -> Option<(usize, &'b K, &'b Option<V>)>
  where K: Borrow<Q>, Q: Eq
  {
    self.entries.iter().enumerate().find_map(|(i, (k, v))| {
      if query.eq((*k).borrow()) { Some((i, k, v)) } else { None }
    })
  }

  /// Find entry in prototype chain in O(n)
  pub fn get<'b, Q: ?Sized>(&'b self, query: &Q) -> Option<&'b V>
  where K: Borrow<Q>, Q: Eq
  {
    if let Some((_, _, v)) = self.local_entry(query) {
      v.as_ref()
    } else {
      self.prototype?.get(query)
    }
  }

  /// Record a value for the given key in O(m)
  pub fn set(&mut self, key: &K, value: V) where K: Eq + Clone {
    if let Some((_, _, v)) = self.local_entry_mut(key) {
      *v = Some(value);
    } else {
      self.entries.push((key.clone(), Some(value)))
    }
  }

  /// Delete in a memory-efficient way in O(n)
  pub fn delete_small(&mut self, key: &K) where K: Eq + Clone {
    let exists_up = self.prototype.and_then(|p| p.get(key)).is_some();
    let local_entry = self.local_entry_mut(key);
    match (exists_up, local_entry) {
      (false, None) => (), // nothing to do
      (false, Some((i, _, _))) => { self.entries.remove(i); }, // forget locally
      (true, Some((_, _, v))) => *v = None, // update local override to cover
      (true, None) => self.entries.push((key.clone(), None)), // create new
    }
  }

  /// Delete in O(m) without checking the prototype chain
  /// May produce unnecessary cover over previously unknown key
  pub fn delete_fast(&mut self, key: &K) where K: Eq + Clone {
    if let Some((_, _, v)) = self.local_entry_mut(key) {
      *v = None
    } else {
      self.entries.push((key.clone(), None))
    }
  }

  /// Iterate over the values defined herein and on the prototype chain
  /// Note that this will visit keys multiple times
  pub fn iter(&self) -> impl Iterator<Item = &(K, Option<V>)> {
    let mut map = self;
    iter::from_fn(move || {
      let pairs = map.entries.iter();
      map = map.prototype?;
      Some(pairs)
    }).flatten()
  }

  /// Visit the keys in an unsafe random order, repeated arbitrarily many times
  pub fn keys(&self) -> impl Iterator<Item = &K> {
    self.iter().map(|(k, _)| k)
  }

  /// Visit the values in random order
  pub fn values(&self) -> impl Iterator<Item = &V> {
    self.iter().filter_map(|(_, v)| v.as_ref())
  }

  /// Update the prototype, and correspondingly the lifetime of the map
  pub fn set_proto<'b>(self, proto: &'b ProtoMap<'b, K, V, STACK_COUNT>)
  -> ProtoMap<'b, K, V, STACK_COUNT> {
    ProtoMap {
      entries: self.entries,
      prototype: Some(proto)
    }
  }
}

impl<T, K, V, const STACK_COUNT: usize>
From<T> for ProtoMap<'_, K, V, STACK_COUNT>
where T: IntoIterator<Item = (K, V)> {
  fn from(value: T) -> Self {
    Self {
      entries: value.into_iter().map(|(k, v)| (k, Some(v))).collect(),
      prototype: None
    }
  }
}

impl<Q: ?Sized, K, V, const STACK_COUNT: usize>
Index<&Q> for ProtoMap<'_, K, V, STACK_COUNT>
where K: Borrow<Q>, Q: Eq {
  type Output = V;
  fn index(&self, index: &Q) -> &Self::Output {
    self.get(index).expect("Index not found in map")
  }
}

impl<K: Clone, V: Clone, const STACK_COUNT: usize>
Clone for ProtoMap<'_, K, V, STACK_COUNT> {
  fn clone(&self) -> Self {
    Self {
      entries: self.entries.clone(),
      prototype: self.prototype
    }
  }
}

impl<'a, K: 'a, V: 'a, const STACK_COUNT: usize>
Add<(K, V)> for &'a ProtoMap<'a, K, V, STACK_COUNT> {
  type Output = ProtoMap<'a, K, V, STACK_COUNT>;
  fn add(self, rhs: (K, V)) -> Self::Output {
    ProtoMap::from([rhs]).set_proto(self)
  }
}

#[macro_export]
macro_rules! protomap {
  ($($ent:expr),*) => {
    ProtoMap::from([$($ent:expr),*])
  };
}
