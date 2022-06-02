use std::collections::HashMap;
use std::hash::Hash;

/// Cache the return values of an effectless closure in a hashmap
/// Inspired by the closure_cacher crate.
pub struct Cache<I, O, F> {
    store: HashMap<I, O>,
    closure: F
}

impl<I, O, F> Cache<I, O, F> where 
    I: Eq + Hash,
    F: FnMut(I) -> O
{
    pub fn new(closure: F) -> Self {
        Self { store: HashMap::new(), closure }
    }
    pub fn by_copy(&mut self, i: I) -> &O where I: Copy {
        let closure = &mut self.closure;
        self.store.entry(i).or_insert_with(|| closure(i))
    }
    pub fn by_clone(&mut self, i: I) -> &O where I: Clone {
        let closure = &mut self.closure;
        // Make sure we only clone if necessary
        self.store.entry(i).or_insert_with_key(|k| closure(k.clone()))
    }
    pub fn known(&self, i: &I) -> Option<&O> {
        self.store.get(i)
    }
    /// Forget the output for the given input
    pub fn drop(&mut self, i: &I) -> bool {
        self.store.remove(i).is_some()
    } 
}