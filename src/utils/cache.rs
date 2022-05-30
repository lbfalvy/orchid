use std::{collections::HashMap, hash::Hash};

/// Cache the return values of an effectless closure in a hashmap
/// Inspired by the closure_cacher crate.
pub struct Cache<I, O, F> where F: FnMut(I) -> O {
    store: HashMap<I, O>,
    closure: F
}

impl<I, O, F> Cache<I, O, F>
where
    F: FnMut(I) -> O,
    I: Eq + Hash + Copy
{
    pub fn new(closure: F) -> Self { Self { store: HashMap::new(), closure } }
    pub fn get(&mut self, i: I) -> &O {
        // I copied it because I might need `drop` and I prefer `I` to be unconstrained. 
        let closure = &mut self.closure;
        self.store.entry(i).or_insert_with(|| closure(i))
    }
    /// Forget the output for the given input
    pub fn drop(&mut self, i: &I) -> bool {
        self.store.remove(i).is_some()
    } 
}