// use std::{collections::HashSet, hash::Hash};

// use hashbrown::HashMap;

// #[derive(Copy, Clone)]
// pub struct Interned<'a, T> {
//   interner: &'a Interner<T>,
//   data: &'a T,
// }

// impl<'a, T: Eq> Eq for Interned<'a, T> {}
// impl<'a, T: PartialEq> PartialEq for Interned<'a, T> {
//   fn eq(&self, other: &Self) -> bool {
//     if (self.interner as *const _) == (other.interner as *const _) {
//       (self.data as *const _) == (other.data as *const _)
//     } else {self.data == other.data}
//   }
// }

// pub struct Interner<T> {
//   data: HashSet<T>,
//   hash_cache: HashMap<>
// }

// impl Interner<T> {

// }