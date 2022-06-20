use std::hash::Hash;
use hashbrown::HashMap;

/// Cache the return values of an effectless closure in a hashmap
/// Inspired by the closure_cacher crate.
pub struct Cache<I, O, F> {
    store: HashMap<I, O>,
    closure: F
}

impl<I: 'static, O, F> Cache<I, O, F> where 
    I: Eq + Hash,
    F: FnMut(I) -> O
{
    pub fn new(closure: F) -> Self {
        Self { store: HashMap::new(), closure }
    }
    /// Produce and cache a result by copying I if necessary
    pub fn by_copy(&mut self, i: &I) -> &O where I: Copy {
        let closure = &mut self.closure;
        self.store.raw_entry_mut().from_key(i)
            .or_insert_with(|| (*i, closure(*i))).1
    }
    /// Produce and cache a result by cloning I if necessary
    pub fn by_clone(&mut self, i: &I) -> &O where I: Clone {
        let closure = &mut self.closure;
        self.store.raw_entry_mut().from_key(i)
            .or_insert_with(|| (i.clone(), closure(i.clone()))).1
    }
    /// Return the result if it has already been computed
    pub fn known(&self, i: &I) -> Option<&O> {
        self.store.get(i)
    }
    /// Forget the output for the given input
    pub fn drop(&mut self, i: &I) -> bool {
        self.store.remove(i).is_some()
    }
}

impl<I: 'static, O, E, F> Cache<I, Result<O, E>, F> where 
    I: Eq + Hash,
    E: Clone,
    F: FnMut(I) -> Result<O, E>
{
    /// Sink the ref from a Result into the Ok value, such that copying only occurs on the sad path
    /// but the return value can be short-circuited
    pub fn by_copy_fallible(&mut self, i: &I) -> Result<&O, E> where I: Copy {
        self.by_clone(i).as_ref().map_err(|e| e.clone())
    }
    /// Sink the ref from a Result into the Ok value, such that cloning only occurs on the sad path
    /// but the return value can be short-circuited
    pub fn by_clone_fallible(&mut self, i: &I) -> Result<&O, E> where I: Clone {
        self.by_clone(i).as_ref().map_err(|e| e.clone())
    }
}

impl<I: 'static, O, F> Cache<I, Option<O>, F> where 
    I: Eq + Hash,
    F: FnMut(I) -> Option<O>
{
    /// Sink the ref from an Option into the Some value such that the return value can be
    /// short-circuited
    pub fn by_copy_fallible(&mut self, i: &I) -> Option<&O> where I: Copy {
        self.by_copy(i).as_ref()
    }
    /// Sink the ref from an Option into the Some value such that the return value can be
    /// short-circuited
    pub fn by_clone_fallible(&mut self, i: &I) -> Option<&O> where I: Clone {
        self.by_clone(i).as_ref()
    } 
}
