use std::ops::{Add, Index};

use hashbrown::{HashMap, hash_map::IntoIter};
use mappable_rc::Mrc;

use crate::expression::Expr;

/// A bucket of indexed expression fragments. Addition may fail if there's a conflict.
#[derive(PartialEq, Eq)]
pub struct State(HashMap<String, Mrc<Vec<Expr>>>);

/// Clone without also cloning arbitrarily heavy Expr objects.
/// Key is expected to be a very short string with an allocator overhead close to zero.
impl Clone for State {
    fn clone(&self) -> Self {
        Self(HashMap::from_iter(
            self.0.iter().map(|(k, v)| (k.clone(), Mrc::clone(v)))
        ))
    }
}

impl State {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
    /// Insert a new element, return None on conflict, clone only on success
    pub fn insert<S>(mut self, k: &S, v: &[Expr]) -> Option<State>
    where S: AsRef<str> + ToString + ?Sized {
        if let Some(old) = self.0.get(k.as_ref()) {
            if old.as_ref() != v {return None}
        } else {
            self.0.insert(k.to_string(), Mrc::new(v.to_vec()));
        }
        return Some(self)
    }
    /// Insert a new entry, return None on conflict
    pub fn insert_pair(mut self, (k, v): (String, Mrc<Vec<Expr>>)) -> Option<State> {
        if let Some(old) = self.0.get(&k) {
            if old != &v {return None}
        } else {
            self.0.insert(k, v);
        }
        return Some(self)
    }
    /// Returns `true` if the state contains no data
    pub fn empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Add for State {
    type Output = Option<State>;

    fn add(mut self, rhs: Self) -> Self::Output {
        if self.empty() {
            return Some(rhs)
        }
        for pair in rhs.0 {
            self = self.insert_pair(pair)?
        }
        return Some(self);
    }
}

impl Add<Option<State>> for State {
    type Output = Option<State>;

    fn add(self, rhs: Option<State>) -> Self::Output {
        rhs.and_then(|s| self + s)
    }
}

impl<'a, S> Index<&S> for State where S: AsRef<str> {
    type Output = Vec<Expr>;

    fn index(&self, index: &S) -> &Self::Output {
        return &self.0[index.as_ref()]
    }
}

impl IntoIterator for State {
    type Item = (String, Mrc<Vec<Expr>>);

    type IntoIter = IntoIter<String, Mrc<Vec<Expr>>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}