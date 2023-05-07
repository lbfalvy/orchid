use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;
use hashbrown::HashMap;

// TODO: make this a crate

/// Cache the return values of an effectless closure in a hashmap
/// Inspired by the closure_cacher crate.
pub struct Cache<'a, I, O: 'static> {
  store: RefCell<HashMap<I, O>>,
  closure: Box<dyn Fn (I, &Self) -> O + 'a>
}

impl<'a, I, O> Cache<'a, I, O> where 
  I: Eq + Hash + Clone, O: Clone
{
  pub fn new<F: 'a>(closure: F) -> Self where F: Fn(I, &Self) -> O {
    Self {
      store: RefCell::new(HashMap::new()),
      closure: Box::new(closure)
    }
  }

  #[allow(unused)]
  pub fn rc<F: 'a>(closure: F) -> Rc<Self> where F: Fn(I, &Self) -> O {
    Rc::new(Self::new(closure))
  }

  /// Produce and cache a result by cloning I if necessary
  pub fn find(&self, i: &I) -> O {
    let closure = &self.closure;
    if let Some(v) = self.store.borrow().get(i) {
      return v.clone()
    }
    // In the moment of invocation the refcell is on immutable
    // this is important for recursive calculations
    let result = closure(i.clone(), self);
    let mut store = self.store.borrow_mut();
    store.raw_entry_mut().from_key(i)
      .or_insert_with(|| (i.clone(), result)).1.clone()
  }

  #[allow(dead_code)]
  /// Return the result if it has already been computed
  pub fn known(&self, i: &I) -> Option<O> {
    let store = self.store.borrow();
    store.get(i).cloned()
  }

  
  /// Convert this cache into a cached [Fn(&I) -> O]
  #[allow(unused)]
  pub fn into_fn(self) -> impl Fn(&I) -> O + 'a where I: 'a {
    move |i| self.find(i)
  }

  /// Borrow this cache with a cached [Fn(&I) -> O]
  #[allow(unused)]
  pub fn as_fn<'b: 'a>(&'b self) -> impl Fn(&I) -> O + 'b where I: 'b {
    move |i| self.find(i)
  }
}

impl<'a, I, O> IntoIterator for Cache<'a, I, O> {
  type IntoIter = hashbrown::hash_map::IntoIter<I, O>;
  type Item = (I, O);
  fn into_iter(self) -> Self::IntoIter {
    let Cache{ store, .. } = self;
    let map = store.into_inner();
    map.into_iter()
  }
}