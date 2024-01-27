use std::borrow::Borrow;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::num::NonZeroUsize;
use std::ops::Index;
use std::vec;

use intern_all::{i, Tok};
use itertools::Itertools;

use crate::utils::boxed_iter::BoxedIter;

/// A borrowed name fragment which can be empty. See [VPath] for the owned
/// variant.
pub struct PathSlice<'a>(pub &'a [Tok<String>]);
impl<'a> PathSlice<'a> {
  /// Convert to an owned name fragment
  pub fn to_vpath(&self) -> VPath { VPath(self.0.to_vec()) }
  /// Iterate over the segments
  pub fn str_iter(&self) -> impl Iterator<Item = &'_ str> {
    Box::new(self.0.iter().map(|s| s.as_str()))
  }
}
impl<'a> Debug for PathSlice<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "VName({self})")
  }
}
impl<'a> Display for PathSlice<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.str_iter().join("::"))
  }
}
impl<'a> Borrow<[Tok<String>]> for PathSlice<'a> {
  fn borrow(&self) -> &[Tok<String>] { self.0 }
}
impl<'a, T> Index<T> for PathSlice<'a>
where [Tok<String>]: Index<T>
{
  type Output = <[Tok<String>] as Index<T>>::Output;

  fn index(&self, index: T) -> &Self::Output { &self.0[index] }
}

/// A token path which may be empty. [VName] is the non-empty,
/// [PathSlice] is the borrowed version
#[derive(Clone, Default, Hash, PartialEq, Eq)]
pub struct VPath(pub Vec<Tok<String>>);
impl VPath {
  /// Collect segments into a vector
  pub fn new(items: impl IntoIterator<Item = Tok<String>>) -> Self {
    Self(items.into_iter().collect())
  }
  /// Prepend some tokens to the path
  pub fn prefix(self, items: impl IntoIterator<Item = Tok<String>>) -> Self {
    Self(items.into_iter().chain(self.0).collect())
  }
  /// Append some tokens to the path
  pub fn suffix(self, items: impl IntoIterator<Item = Tok<String>>) -> Self {
    Self(self.0.into_iter().chain(items).collect())
  }
  /// Partition the string by `::` namespace separators
  pub fn parse(s: &str) -> Self {
    Self(if s.is_empty() { vec![] } else { s.split("::").map(i).collect() })
  }
  /// Walk over the segments
  pub fn str_iter(&self) -> impl Iterator<Item = &'_ str> {
    Box::new(self.0.iter().map(|s| s.as_str()))
  }
  /// Try to convert into non-empty version
  pub fn into_name(self) -> Result<VName, EmptyNameError> { VName::new(self.0) }
  /// Add a token to the path. Since now we know that it can't be empty, turn it
  /// into a name.
  pub fn as_prefix_of(self, name: Tok<String>) -> VName {
    VName(self.into_iter().chain([name]).collect())
  }
  /// Add a token to the beginning of the. Since now we know that it can't be
  /// empty, turn it into a name.
  pub fn as_suffix_of(self, name: Tok<String>) -> VName {
    VName([name].into_iter().chain(self).collect())
  }
}
impl Debug for VPath {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "VName({self})")
  }
}
impl Display for VPath {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.str_iter().join("::"))
  }
}
impl FromIterator<Tok<String>> for VPath {
  fn from_iter<T: IntoIterator<Item = Tok<String>>>(iter: T) -> Self {
    Self(iter.into_iter().collect())
  }
}
impl IntoIterator for VPath {
  type Item = Tok<String>;
  type IntoIter = vec::IntoIter<Self::Item>;
  fn into_iter(self) -> Self::IntoIter { self.0.into_iter() }
}
impl Borrow<[Tok<String>]> for VPath {
  fn borrow(&self) -> &[Tok<String>] { self.0.borrow() }
}

impl<T> Index<T> for VPath
where Vec<Tok<String>>: Index<T>
{
  type Output = <Vec<Tok<String>> as Index<T>>::Output;

  fn index(&self, index: T) -> &Self::Output { &self.0[index] }
}

/// A mutable representation of a namespaced identifier of at least one segment.
///
/// These names may be relative or otherwise partially processed.
///
/// See also [Sym] for the immutable representation, and [VPath] for possibly
/// empty values
#[derive(Clone, Hash, PartialEq, Eq)]
pub struct VName(Vec<Tok<String>>);
impl VName {
  /// Assert that the sequence isn't empty and wrap it in [VName] to represent
  /// this invariant
  pub fn new(
    items: impl IntoIterator<Item = Tok<String>>,
  ) -> Result<Self, EmptyNameError> {
    let data: Vec<_> = items.into_iter().collect();
    if data.is_empty() { Err(EmptyNameError) } else { Ok(Self(data)) }
  }
  /// Unwrap the enclosed vector
  pub fn into_vec(self) -> Vec<Tok<String>> { self.0 }
  /// Get a reference to the enclosed vector
  pub fn vec(&self) -> &Vec<Tok<String>> { &self.0 }
  /// Mutable access to the underlying vector. To ensure correct results, this
  /// must never be empty.
  pub fn vec_mut(&mut self) -> &mut Vec<Tok<String>> { &mut self.0 }
  /// Intern the name and return a [Sym]
  pub fn to_sym(&self) -> Sym { Sym(i(&self.0)) }
  /// like Slice's split_first, but non-optional and tokens are cheap to clone
  pub fn split_first(&self) -> (Tok<String>, &[Tok<String>]) {
    let (h, t) = self.0.split_first().expect("VName can never be empty");
    (h.clone(), t)
  }
  /// like Slice's split_last, but non-optional and tokens are cheap to clone
  pub fn split_last(&self) -> (Tok<String>, &[Tok<String>]) {
    let (f, b) = self.0.split_last().expect("VName can never be empty");
    (f.clone(), b)
  }
  /// If this name has only one segment, return it
  pub fn as_root(&self) -> Option<Tok<String>> {
    self.0.iter().exactly_one().ok().cloned()
  }
  /// Prepend the segments to this name
  #[must_use = "This is a pure function"]
  pub fn prefix(self, items: impl IntoIterator<Item = Tok<String>>) -> Self {
    Self(items.into_iter().chain(self.0).collect())
  }
  /// Append the segments to this name
  #[must_use = "This is a pure function"]
  pub fn suffix(self, items: impl IntoIterator<Item = Tok<String>>) -> Self {
    Self(self.0.into_iter().chain(items).collect())
  }
  /// Read a `::` separated namespaced name
  pub fn parse(s: &str) -> Result<Self, EmptyNameError> {
    Self::new(VPath::parse(s))
  }
  /// Read a name from a string literal which can be known not to be empty
  pub fn literal(s: &'static str) -> Self {
    Self::parse(s).expect("name literal should not be empty")
  }
  /// Find the longest shared prefix of this name and another sequence
  pub fn coprefix(&self, other: &[Tok<String>]) -> &[Tok<String>] {
    &self.0
      [0..self.0.iter().zip(other.iter()).take_while(|(l, r)| l == r).count()]
  }
  /// Obtain an iterator over the segments of the name
  pub fn iter(&self) -> impl Iterator<Item = Tok<String>> + '_ {
    self.0.iter().cloned()
  }

  /// Convert to [PathSlice]
  pub fn as_path_slice(&self) -> PathSlice { PathSlice(&self[..]) }
}
impl Debug for VName {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "VName({self})")
  }
}
impl Display for VName {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.str_iter().join("::"))
  }
}
impl IntoIterator for VName {
  type Item = Tok<String>;
  type IntoIter = vec::IntoIter<Self::Item>;
  fn into_iter(self) -> Self::IntoIter { self.0.into_iter() }
}
impl<T> Index<T> for VName
where Vec<Tok<String>>: Index<T>
{
  type Output = <Vec<Tok<String>> as Index<T>>::Output;

  fn index(&self, index: T) -> &Self::Output { &self.0[index] }
}
impl Borrow<[Tok<String>]> for VName {
  fn borrow(&self) -> &[Tok<String>] { self.0.borrow() }
}

/// Error produced when a non-empty name [VName] or [Sym] is constructed with an
/// empty sequence
#[derive(Debug, Copy, Clone, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct EmptyNameError;
impl TryFrom<&[Tok<String>]> for VName {
  type Error = EmptyNameError;
  fn try_from(value: &[Tok<String>]) -> Result<Self, Self::Error> {
    Self::new(value.iter().cloned())
  }
}

/// An interned representation of a namespaced identifier.
///
/// These names are always absolute.
///
/// See also [VName]
#[derive(Clone, Hash, PartialEq, Eq)]
pub struct Sym(Tok<Vec<Tok<String>>>);
impl Sym {
  /// Assert that the sequence isn't empty, intern it and wrap it in a [Sym] to
  /// represent this invariant
  pub fn new(
    v: impl IntoIterator<Item = Tok<String>>,
  ) -> Result<Self, EmptyNameError> {
    let items = v.into_iter().collect::<Vec<_>>();
    Self::from_tok(i(&items))
  }

  /// Read a `::` separated namespaced name.
  pub fn parse(s: &str) -> Result<Self, EmptyNameError> {
    Ok(Sym(i(&VName::parse(s)?.into_vec())))
  }

  /// Parse a string and panic if it's not empty
  pub fn literal(s: &'static str) -> Self {
    Self::parse(s).expect("name literal should not be empty")
  }

  /// Assert that a token isn't empty, and wrap it in a [Sym]
  pub fn from_tok(t: Tok<Vec<Tok<String>>>) -> Result<Self, EmptyNameError> {
    if t.is_empty() { Err(EmptyNameError) } else { Ok(Self(t)) }
  }
  /// Grab the interner token
  pub fn tok(&self) -> Tok<Vec<Tok<String>>> { self.0.clone() }
  /// Get a number unique to this name suitable for arbitrary ordering.
  pub fn id(&self) -> NonZeroUsize { self.0.id() }
  /// Get an iterator over the tokens in this name
  pub fn iter(&self) -> impl Iterator<Item = Tok<String>> + '_ {
    self.0.iter().cloned()
  }

  /// Like Slice's split_last, except this slice is never empty
  pub fn split_last(&self) -> (Tok<String>, PathSlice) {
    let (foot, torso) = self.0.split_last().expect("Sym never empty");
    (foot.clone(), PathSlice(torso))
  }

  /// Like Slice's split_first, except this slice is never empty
  pub fn split_first(&self) -> (Tok<String>, PathSlice) {
    let (head, tail) = self.0.split_first().expect("Sym never empty");
    (head.clone(), PathSlice(tail))
  }

  /// Extern the sym for editing
  pub fn to_vname(&self) -> VName { VName(self[..].to_vec()) }

  /// Convert to [PathSlice]
  pub fn as_path_slice(&self) -> PathSlice { PathSlice(&self[..]) }
}
impl Debug for Sym {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Sym{self})")
  }
}
impl Display for Sym {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.str_iter().join("::"))
  }
}
impl<T> Index<T> for Sym
where Vec<Tok<String>>: Index<T>
{
  type Output = <Vec<Tok<String>> as Index<T>>::Output;

  fn index(&self, index: T) -> &Self::Output { &(&*self.0)[index] }
}
impl Borrow<[Tok<String>]> for Sym {
  fn borrow(&self) -> &[Tok<String>] { self.0.borrow() }
}

/// An abstraction over tokenized vs non-tokenized names so that they can be
/// handled together in datastructures. The names can never be empty
#[allow(clippy::len_without_is_empty)] // never empty
pub trait NameLike: 'static + Clone + Eq + Hash + Debug + Display {
  /// Fully resolve the name for printing
  #[must_use]
  fn to_strv(&self) -> Vec<String> {
    self.str_iter().map(str::to_owned).collect()
  }
  /// Format the name as an approximate filename
  fn as_src_path(&self) -> String {
    format!("{}.orc", self.str_iter().join("/"))
  }
  /// Return the number of segments in the name
  fn len(&self) -> NonZeroUsize {
    NonZeroUsize::try_from(self.str_iter().count())
      .expect("NameLike never empty")
  }
  /// Fully resolve the name for printing
  fn str_iter(&self) -> BoxedIter<'_, &str>;
}

impl NameLike for Sym {
  fn str_iter(&self) -> BoxedIter<'_, &str> {
    Box::new(self.0.iter().map(|s| s.as_str()))
  }
}

impl NameLike for VName {
  fn str_iter(&self) -> BoxedIter<'_, &str> {
    Box::new(self.0.iter().map(|s| s.as_str()))
  }
}
