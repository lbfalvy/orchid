use std::cell::RefCell;
use std::hash::Hash;

use hashbrown::HashMap;
use trait_set::trait_set;

// TODO: make this a crate
trait_set! {
  pub trait Callback<'a, I, O: 'static> = Fn(I, &Cache<'a, I, O>) -> O;
}
pub type CbBox<'a, I, O> = Box<dyn Callback<'a, I, O> + 'a>;

/// Cache the return values of an effectless closure in a hashmap
/// Inspired by the closure_cacher crate.
pub struct Cache<'a, I, O: 'static> {
  store: RefCell<HashMap<I, O>>,
  closure: CbBox<'a, I, O>,
}

impl<'a, I: Eq + Hash + Clone, O: Clone> Cache<'a, I, O> {
  pub fn new<F: 'a + Callback<'a, I, O>>(closure: F) -> Self {
    Self { store: RefCell::new(HashMap::new()), closure: Box::new(closure) }
  }

  /// Produce and cache a result by cloning I if necessary
  pub fn find(&self, i: &I) -> O {
    let closure = &self.closure;
    if let Some(v) = self.store.borrow().get(i) {
      return v.clone();
    }
    // In the moment of invocation the refcell is on immutable
    // this is important for recursive calculations
    let result = closure(i.clone(), self);
    let mut store = self.store.borrow_mut();
    store
      .raw_entry_mut()
      .from_key(i)
      .or_insert_with(|| (i.clone(), result))
      .1
      .clone()
  }
}

impl<'a, I, O> IntoIterator for Cache<'a, I, O> {
  type IntoIter = hashbrown::hash_map::IntoIter<I, O>;
  type Item = (I, O);
  fn into_iter(self) -> Self::IntoIter {
    let Cache { store, .. } = self;
    let map = store.into_inner();
    map.into_iter()
  }
}
