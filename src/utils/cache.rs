use std::{hash::Hash, cell::RefCell, rc::Rc};
use hashbrown::HashMap;
use mappable_rc::Mrc;

/// Convenience trait for overriding Mrc's strange cloning logic
pub trait MyClone {
  fn my_clone(&self) -> Self;
}

impl<T> MyClone for T where T: Clone {
  default fn my_clone(&self) -> Self { self.clone() }
}

impl<T: ?Sized> MyClone for Rc<T> {
  fn my_clone(&self) -> Self { Rc::clone(self) }
}
impl<T: ?Sized> MyClone for Mrc<T> {
  fn my_clone(&self) -> Self { Mrc::clone(self) }
}

/// Cache the return values of an effectless closure in a hashmap
/// Inspired by the closure_cacher crate.
pub struct Cache<'a, I, O: 'static> {
  store: RefCell<HashMap<I, Mrc<O>>>,
  closure: Box<dyn Fn (I, &Self) -> Mrc<O> + 'a>
}

impl<'a, I, O> Cache<'a, I, O> where 
  I: Eq + Hash + MyClone
{
  pub fn new<F: 'a>(closure: F) -> Self where F: Fn(I, &Self) -> O {
    Self::new_raw(move |o, s| Mrc::new(closure(o, s)))
  }

  /// Take an Mrc<O> closure rather than an O closure
  /// Used internally to derive caches from other systems working with Mrc-s
  pub fn new_raw<F: 'a>(closure: F) -> Self where F: Fn(I, &Self) -> Mrc<O> {
    Self {
      store: RefCell::new(HashMap::new()),
      closure: Box::new(closure)
    }
  }

  /// Produce and cache a result by cloning I if necessary
  pub fn find(&self, i: &I) -> Mrc<O> {
    let closure = &self.closure;
    if let Some(v) = self.store.borrow().get(i) {
      return Mrc::clone(v)
    }
    // In the moment of invocation the refcell is on immutable
    // this is important for recursive calculations
    let result = closure(i.my_clone(), self);
    let mut store = self.store.borrow_mut();
    Mrc::clone(store.raw_entry_mut().from_key(i)
      .or_insert_with(|| (i.my_clone(), result)).1)
  }

  #[allow(dead_code)]
  /// Return the result if it has already been computed
  pub fn known(&self, i: &I) -> Option<Mrc<O>> {
    let store = self.store.borrow();
    store.get(i).map(Mrc::clone)
  }
  #[allow(dead_code)]
  /// Forget the output for the given input
  pub fn drop(&self, i: &I) -> bool {
    self.store.borrow_mut().remove(i).is_some()
  }
}

impl<'a, I, O, E> Cache<'a, I, Result<O, E>> where 
  I: Eq + Hash + MyClone,
  // O: Clone,
  E: Clone
{
  /// Sink the ref from a Result into the Ok value, such that cloning only occurs on the sad path
  /// but the return value can be short-circuited
  pub fn try_find(&self, i: &I) -> Result<Mrc<O>, E> {
    let ent = self.find(i);
    Mrc::try_map(ent, |t| t.as_ref().ok())
    .map_err(|res| Result::as_ref(&res).err().unwrap().to_owned())
  }
}

impl<'a, I, O> Cache<'a, I, Option<O>> where 
  I: Eq + Hash + MyClone,
  // O: Clone
{
  #[allow(dead_code)]
  /// Sink the ref from an Option into the Some value such that the return value can be
  /// short-circuited
  pub fn try_find(&self, i: &I) -> Option<Mrc<O>> where I: Clone {
    let ent = self.find(i);
    Mrc::try_map(ent, |o| o.as_ref()).ok()
  } 
}
