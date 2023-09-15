use std::fmt::Debug;
use std::hash::Hash;

use crate::interner::{Interner, Tok};

/// A mutable representation of a namespaced identifier.
///
/// These names may be relative or otherwise partially processed.
///
/// See also [Sym]
pub type VName = Vec<Tok<String>>;

/// An interned representation of a namespaced identifier.
///
/// These names are always absolute.
///
/// See also [VName]
pub type Sym = Tok<VName>;

/// An abstraction over tokenized vs non-tokenized names so that they can be
/// handled together in datastructures
pub trait NameLike: 'static + Clone + Eq + Hash + Debug {
  /// Fully resolve the name for printing
  fn to_strv(&self) -> Vec<String>;
}

impl NameLike for Sym {
  fn to_strv(&self) -> Vec<String> { self.extern_vec() }
}

impl NameLike for VName {
  fn to_strv(&self) -> Vec<String> { Interner::extern_all(self) }
}
