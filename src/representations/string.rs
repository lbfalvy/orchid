use std::hash::Hash;
use std::ops::Deref;
use std::rc::Rc;

use crate::Tok;

#[derive(Clone, Debug, Eq)]
pub enum OrcString {
  Interned(Tok<String>),
  Runtime(Rc<String>),
}

impl OrcString {
  pub fn get_string(&self) -> String {
    self.as_str().to_owned()
  }
}

impl Deref for OrcString {
  type Target = String;

  fn deref(&self) -> &Self::Target {
    match self {
      Self::Interned(t) => t,
      Self::Runtime(r) => r,
    }
  }
}

impl Hash for OrcString {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.as_str().hash(state)
  }
}

impl From<String> for OrcString {
  fn from(value: String) -> Self {
    Self::Runtime(Rc::new(value))
  }
}

impl From<Tok<String>> for OrcString {
  fn from(value: Tok<String>) -> Self {
    Self::Interned(value)
  }
}

impl PartialEq for OrcString {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Interned(t1), Self::Interned(t2)) => t1 == t2,
      _ => **self == **other,
    }
  }
}
