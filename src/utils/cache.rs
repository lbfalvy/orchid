use std::{hash::Hash, cell::RefCell};
use hashbrown::HashMap;
use mappable_rc::Mrc;

/// Cache the return values of an effectless closure in a hashmap
/// Inspired by the closure_cacher crate.
pub struct Cache<I, O: 'static> where O: Clone {
    store: RefCell<HashMap<I, Mrc<O>>>,
    closure: RefCell<Box<dyn FnMut (I) -> O + 'static>>
}

impl<I, O> Cache<I, O> where 
    I: Eq + Hash + Clone,
    O: Clone
{
    pub fn new<F: 'static>(closure: F) -> Self where F: FnMut(I) -> O {
        Self {
            store: RefCell::new(HashMap::new()),
            closure: RefCell::new(Box::new(closure))
        }
    }

    /// Produce and cache a result by cloning I if necessary
    pub fn find(&self, i: &I) -> Mrc<O> {
        let mut closure = self.closure.borrow_mut();
        let mut store = self.store.borrow_mut();
        Mrc::clone(store.raw_entry_mut().from_key(i)
            .or_insert_with(|| (i.clone(), Mrc::new(closure(i.clone())))).1)
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

impl<I, O, E> Cache<I, Result<O, E>> where 
    I: Eq + Hash + Clone,
    O: Clone,
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

impl<I, O> Cache<I, Option<O>> where 
    I: Eq + Hash + Clone,
    O: Clone
{
    #[allow(dead_code)]
    /// Sink the ref from an Option into the Some value such that the return value can be
    /// short-circuited
    pub fn try_find(&self, i: &I) -> Option<Mrc<O>> where I: Clone {
        let ent = self.find(i);
        Mrc::try_map(ent, |o| o.as_ref()).ok()
    } 
}
