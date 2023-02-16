use std::{ops::{Add, Index}, rc::Rc, fmt::Debug};

use hashbrown::HashMap;

use crate::ast::Expr;

#[derive(Debug, PartialEq, Eq)]
pub enum Entry {
  Vec(Rc<Vec<Expr>>),
  Scalar(Rc<Expr>),
  Name(Rc<String>),
  NameOpt(Option<Rc<String>>)
}

/// A bucket of indexed expression fragments. Addition may fail if there's a conflict.
#[derive(PartialEq, Eq, Clone)]
pub struct State(HashMap<String, Entry>);

/// Clone without also cloning arbitrarily heavy Expr objects.
/// Key is expected to be a very short string with an allocator overhead close to zero.
impl Clone for Entry {
  fn clone(&self) -> Self {
    match self {
      Self::Name(n) => Self::Name(Rc::clone(n)),
      Self::Scalar(x) => Self::Scalar(Rc::clone(x)),
      Self::Vec(v) => Self::Vec(Rc::clone(v)),
      Self::NameOpt(o) => Self::NameOpt(o.as_ref().map(Rc::clone))
    }
  }
}

impl State {
  pub fn new() -> Self {
    Self(HashMap::new())
  }
  pub fn insert_vec<S>(mut self, k: &S, v: &[Expr]) -> Option<Self>
  where S: AsRef<str> + ToString + ?Sized + Debug {
    if let Some(old) = self.0.get(k.as_ref()) {
      if let Entry::Vec(val) = old {
        if val.as_slice() != v {return None}
      } else {return None}
    } else {
      self.0.insert(k.to_string(), Entry::Vec(Rc::new(v.to_vec())));
    }
    Some(self)
  }
  pub fn insert_scalar<S>(mut self, k: &S, v: &Expr) -> Option<Self>
  where S: AsRef<str> + ToString + ?Sized {
    if let Some(old) = self.0.get(k.as_ref()) {
      if let Entry::Scalar(val) = old {
        if val.as_ref() != v {return None}
      } else {return None}
    } else {
      self.0.insert(k.to_string(), Entry::Scalar(Rc::new(v.to_owned())));
    }
    Some(self)
  }
  pub fn insert_name<S1, S2>(mut self, k: &S1, v: &S2) -> Option<Self>
  where
    S1: AsRef<str> + ToString + ?Sized,
    S2: AsRef<str> + ToString + ?Sized
  {
    if let Some(old) = self.0.get(k.as_ref()) {
      if let Entry::Name(val) = old {
        if val.as_str() != v.as_ref() {return None}
      } else {return None}
    } else {
      self.0.insert(k.to_string(), Entry::Name(Rc::new(v.to_string())));
    }
    Some(self)
  }
  pub fn insert_name_opt<S1, S2>(mut self, k: &S1, v: Option<&S2>) -> Option<Self>
  where
    S1: AsRef<str> + ToString + ?Sized,
    S2: AsRef<str> + ToString + ?Sized
  {
    if let Some(old) = self.0.get(k.as_ref()) {
      if let Entry::NameOpt(val) = old {
        if val.as_ref().map(|s| s.as_ref().as_str()) != v.map(|s| s.as_ref()) {
          return None
        }
      } else {return None}
    } else {
      self.0.insert(k.to_string(), Entry::NameOpt(v.map(|s| Rc::new(s.to_string()))));
    }
    Some(self)
  }
  /// Insert a new entry, return None on conflict
  pub fn insert_pair(mut self, (k, v): (String, Entry)) -> Option<State> {
    if let Some(old) = self.0.get(&k) {
      if old != &v {return None}
    } else {
      self.0.insert(k, v);
    }
    Some(self)
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
    Some(self)
  }
}

impl Add<Option<State>> for State {
  type Output = Option<State>;

  fn add(self, rhs: Option<State>) -> Self::Output {
    rhs.and_then(|s| self + s)
  }
}

impl<S> Index<S> for State where S: AsRef<str> {
  type Output = Entry;

  fn index(&self, index: S) -> &Self::Output {
    return &self.0[index.as_ref()]
  }
}

impl IntoIterator for State {
  type Item = (String, Entry);

  type IntoIter = hashbrown::hash_map::IntoIter<String, Entry>;

  fn into_iter(self) -> Self::IntoIter {
    self.0.into_iter()
  }
}

impl Debug for State {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self.0)
  }
}