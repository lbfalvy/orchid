//! Various datatypes that all represent namespaced names.

use std::borrow::Borrow;
use std::hash::Hash;
use std::iter::Cloned;
use std::num::{NonZeroU64, NonZeroUsize};
use std::ops::{Bound, Deref, Index, RangeBounds};
use std::path::Path;
use std::{fmt, slice, vec};

use futures::future::{OptionFuture, join_all};
use itertools::Itertools;
use trait_set::trait_set;

use crate::api;
use crate::interner::{InternMarker, Interner, Tok};

trait_set! {
	/// Traits that all name iterators should implement
	pub trait NameIter = Iterator<Item = Tok<String>> + DoubleEndedIterator + ExactSizeIterator;
}

/// A borrowed name fragment which can be empty. See [VPath] for the owned
/// variant.
#[derive(Hash, PartialEq, Eq)]
#[repr(transparent)]
pub struct PathSlice([Tok<String>]);
impl PathSlice {
	/// Create a new [PathSlice]
	pub fn new(slice: &[Tok<String>]) -> &PathSlice {
		// SAFETY: This is ok because PathSlice is #[repr(transparent)]
		unsafe { &*(slice as *const [Tok<String>] as *const PathSlice) }
	}
	/// Convert to an owned name fragment
	pub fn to_vpath(&self) -> VPath { VPath(self.0.to_vec()) }
	/// Iterate over the tokens
	pub fn iter(&self) -> impl NameIter + '_ { self.into_iter() }
	/// Iterate over the segments
	pub fn str_iter(&self) -> impl Iterator<Item = &'_ str> {
		Box::new(self.0.iter().map(|s| s.as_str()))
	}
	/// Find the longest shared prefix of this name and another sequence
	pub fn coprefix<'a>(&'a self, other: &PathSlice) -> &'a PathSlice {
		&self[0..self.iter().zip(other.iter()).take_while(|(l, r)| l == r).count() as u16]
	}
	/// Find the longest shared suffix of this name and another sequence
	pub fn cosuffix<'a>(&'a self, other: &PathSlice) -> &'a PathSlice {
		&self[0..self.iter().zip(other.iter()).take_while(|(l, r)| l == r).count() as u16]
	}
	/// Remove another
	pub fn strip_prefix<'a>(&'a self, other: &PathSlice) -> Option<&'a PathSlice> {
		let shared = self.coprefix(other).len();
		(shared == other.len()).then_some(PathSlice::new(&self[shared..]))
	}
	/// Number of path segments
	pub fn len(&self) -> u16 { self.0.len().try_into().expect("Too long name!") }
	pub fn get<I: NameIndex>(&self, index: I) -> Option<&I::Output> { index.get(self) }
	/// Whether there are any path segments. In other words, whether this is a
	/// valid name
	pub fn is_empty(&self) -> bool { self.len() == 0 }
	/// Obtain a reference to the held slice. With all indexing traits shadowed,
	/// this is better done explicitly
	pub fn as_slice(&self) -> &[Tok<String>] { self }
	/// Global empty path slice
	pub fn empty() -> &'static Self { PathSlice::new(&[]) }
}
impl fmt::Debug for PathSlice {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "VName({self})") }
}
impl fmt::Display for PathSlice {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.str_iter().join("::"))
	}
}
impl Borrow<[Tok<String>]> for PathSlice {
	fn borrow(&self) -> &[Tok<String>] { &self.0 }
}
impl<'a> IntoIterator for &'a PathSlice {
	type IntoIter = Cloned<slice::Iter<'a, Tok<String>>>;
	type Item = Tok<String>;
	fn into_iter(self) -> Self::IntoIter { self.0.iter().cloned() }
}

pub trait NameIndex {
	type Output: ?Sized;
	fn get(self, name: &PathSlice) -> Option<&Self::Output>;
}
impl<T: NameIndex> Index<T> for PathSlice {
	type Output = T::Output;
	fn index(&self, index: T) -> &Self::Output { index.get(self).expect("Index out of bounds") }
}

mod idx_impls {
	use std::ops;

	use super::{NameIndex, PathSlice, conv_range};
	use crate::interner::Tok;

	impl NameIndex for u16 {
		type Output = Tok<String>;
		fn get(self, name: &PathSlice) -> Option<&Self::Output> { name.0.get(self as usize) }
	}

	impl NameIndex for ops::RangeFull {
		type Output = PathSlice;
		fn get(self, name: &PathSlice) -> Option<&Self::Output> { Some(name) }
	}

	macro_rules! impl_range_index_for_pathslice {
		($range:ident) => {
			impl ops::Index<ops::$range<u16>> for PathSlice {
				type Output = Self;
				fn index(&self, index: ops::$range<u16>) -> &Self::Output {
					Self::new(&self.0[conv_range::<u16, usize>(index)])
				}
			}
		};
	}

	impl_range_index_for_pathslice!(RangeFrom);
	impl_range_index_for_pathslice!(RangeTo);
	impl_range_index_for_pathslice!(Range);
	impl_range_index_for_pathslice!(RangeInclusive);
	impl_range_index_for_pathslice!(RangeToInclusive);
}

impl Deref for PathSlice {
	type Target = [Tok<String>];

	fn deref(&self) -> &Self::Target { &self.0 }
}
impl Borrow<PathSlice> for [Tok<String>] {
	fn borrow(&self) -> &PathSlice { PathSlice::new(self) }
}
impl<const N: usize> Borrow<PathSlice> for [Tok<String>; N] {
	fn borrow(&self) -> &PathSlice { PathSlice::new(&self[..]) }
}
impl Borrow<PathSlice> for Vec<Tok<String>> {
	fn borrow(&self) -> &PathSlice { PathSlice::new(&self[..]) }
}
pub fn conv_bound<T: Into<U> + Clone, U>(bound: Bound<&T>) -> Bound<U> {
	match bound {
		Bound::Included(i) => Bound::Included(i.clone().into()),
		Bound::Excluded(i) => Bound::Excluded(i.clone().into()),
		Bound::Unbounded => Bound::Unbounded,
	}
}
pub fn conv_range<'a, T: Into<U> + Clone + 'a, U: 'a>(
	range: impl RangeBounds<T>,
) -> (Bound<U>, Bound<U>) {
	(conv_bound(range.start_bound()), conv_bound(range.end_bound()))
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
	/// Number of path segments
	pub fn len(&self) -> usize { self.0.len() }
	/// Whether there are any path segments. In other words, whether this is a
	/// valid name
	pub fn is_empty(&self) -> bool { self.len() == 0 }
	/// Prepend some tokens to the path
	pub fn prefix(self, items: impl IntoIterator<Item = Tok<String>>) -> Self {
		Self(items.into_iter().chain(self.0).collect())
	}
	/// Append some tokens to the path
	pub fn suffix(self, items: impl IntoIterator<Item = Tok<String>>) -> Self {
		Self(self.0.into_iter().chain(items).collect())
	}
	/// Partition the string by `::` namespace separators
	pub async fn parse(s: &str, i: &Interner) -> Self {
		Self(if s.is_empty() { vec![] } else { join_all(s.split("::").map(|s| i.i(s))).await })
	}
	/// Walk over the segments
	pub fn str_iter(&self) -> impl Iterator<Item = &'_ str> {
		Box::new(self.0.iter().map(|s| s.as_str()))
	}
	/// Try to convert into non-empty version
	pub fn into_name(self) -> Result<VName, EmptyNameError> { VName::new(self.0) }
	/// Add a token to the path. Since now we know that it can't be empty, turn it
	/// into a name.
	pub fn name_with_prefix(self, name: Tok<String>) -> VName {
		VName(self.into_iter().chain([name]).collect())
	}
	/// Add a token to the beginning of the. Since now we know that it can't be
	/// empty, turn it into a name.
	pub fn name_with_suffix(self, name: Tok<String>) -> VName {
		VName([name].into_iter().chain(self).collect())
	}

	/// Convert a fs path to a vpath
	pub async fn from_path(path: &Path, ext: &str, i: &Interner) -> Option<(Self, bool)> {
		async fn to_vpath(p: &Path, i: &Interner) -> Option<VPath> {
			let tok_opt_v =
				join_all(p.iter().map(|c| OptionFuture::from(c.to_str().map(|s| i.i(s))))).await;
			tok_opt_v.into_iter().collect::<Option<_>>().map(VPath)
		}
		match path.extension().map(|s| s.to_str()) {
			Some(Some(s)) if s == ext => Some((to_vpath(&path.with_extension(""), i).await?, true)),
			None => Some((to_vpath(path, i).await?, false)),
			Some(_) => None,
		}
	}
}
impl fmt::Debug for VPath {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "VName({self})") }
}
impl fmt::Display for VPath {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
impl Borrow<PathSlice> for VPath {
	fn borrow(&self) -> &PathSlice { PathSlice::new(&self.0[..]) }
}
impl Deref for VPath {
	type Target = PathSlice;
	fn deref(&self) -> &Self::Target { self.borrow() }
}

impl<T> Index<T> for VPath
where PathSlice: Index<T>
{
	type Output = <PathSlice as Index<T>>::Output;

	fn index(&self, index: T) -> &Self::Output { &Borrow::<PathSlice>::borrow(self)[index] }
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
	pub fn new(items: impl IntoIterator<Item = Tok<String>>) -> Result<Self, EmptyNameError> {
		let data: Vec<_> = items.into_iter().collect();
		if data.is_empty() { Err(EmptyNameError) } else { Ok(Self(data)) }
	}
	pub async fn deintern(
		name: impl IntoIterator<Item = api::TStr>,
		i: &Interner,
	) -> Result<Self, EmptyNameError> {
		Self::new(join_all(name.into_iter().map(|m| Tok::from_api(m, i))).await)
	}
	/// Unwrap the enclosed vector
	pub fn into_vec(self) -> Vec<Tok<String>> { self.0 }
	/// Get a reference to the enclosed vector
	pub fn vec(&self) -> &Vec<Tok<String>> { &self.0 }
	/// Mutable access to the underlying vector. To ensure correct results, this
	/// must never be empty.
	pub fn vec_mut(&mut self) -> &mut Vec<Tok<String>> { &mut self.0 }
	/// Intern the name and return a [Sym]
	pub async fn to_sym(&self, i: &Interner) -> Sym { Sym(i.i(&self.0[..]).await) }
	/// If this name has only one segment, return it
	pub fn as_root(&self) -> Option<Tok<String>> { self.0.iter().exactly_one().ok().cloned() }
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
	pub async fn parse(s: &str, i: &Interner) -> Result<Self, EmptyNameError> {
		Self::new(VPath::parse(s, i).await)
	}
	pub async fn literal(s: &'static str, i: &Interner) -> Self {
		Self::parse(s, i).await.expect("empty literal !?")
	}
	/// Obtain an iterator over the segments of the name
	pub fn iter(&self) -> impl Iterator<Item = Tok<String>> + '_ { self.0.iter().cloned() }
}
impl fmt::Debug for VName {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "VName({self})") }
}
impl fmt::Display for VName {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.str_iter().join("::"))
	}
}
impl IntoIterator for VName {
	type Item = Tok<String>;
	type IntoIter = vec::IntoIter<Self::Item>;
	fn into_iter(self) -> Self::IntoIter { self.0.into_iter() }
}
impl<T> Index<T> for VName
where PathSlice: Index<T>
{
	type Output = <PathSlice as Index<T>>::Output;

	fn index(&self, index: T) -> &Self::Output { &self.deref()[index] }
}
impl Borrow<[Tok<String>]> for VName {
	fn borrow(&self) -> &[Tok<String>] { self.0.borrow() }
}
impl Borrow<PathSlice> for VName {
	fn borrow(&self) -> &PathSlice { PathSlice::new(&self.0[..]) }
}
impl Deref for VName {
	type Target = PathSlice;
	fn deref(&self) -> &Self::Target { self.borrow() }
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
	pub async fn new(
		v: impl IntoIterator<Item = Tok<String>>,
		i: &Interner,
	) -> Result<Self, EmptyNameError> {
		let items = v.into_iter().collect_vec();
		Self::from_tok(i.i(&items).await)
	}
	/// Read a `::` separated namespaced name.
	pub async fn parse(s: &str, i: &Interner) -> Result<Self, EmptyNameError> {
		Ok(Sym(i.i(&VName::parse(s, i).await?.into_vec()).await))
	}
	/// Assert that a token isn't empty, and wrap it in a [Sym]
	pub fn from_tok(t: Tok<Vec<Tok<String>>>) -> Result<Self, EmptyNameError> {
		if t.is_empty() { Err(EmptyNameError) } else { Ok(Self(t)) }
	}
	/// Grab the interner token
	pub fn tok(&self) -> Tok<Vec<Tok<String>>> { self.0.clone() }
	/// Get a number unique to this name suitable for arbitrary ordering.
	pub fn id(&self) -> NonZeroU64 { self.0.to_api().get_id() }
	/// Extern the sym for editing
	pub fn to_vname(&self) -> VName { VName(self[..].to_vec()) }
	pub async fn from_api(marker: api::TStrv, i: &Interner) -> Sym {
		Self::from_tok(Tok::from_api(marker, i).await).expect("Empty sequence found for serialized Sym")
	}
	pub fn to_api(&self) -> api::TStrv { self.tok().to_api() }
}
impl fmt::Debug for Sym {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "Sym({self})") }
}
impl fmt::Display for Sym {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.str_iter().join("::"))
	}
}
impl<T> Index<T> for Sym
where PathSlice: Index<T>
{
	type Output = <PathSlice as Index<T>>::Output;

	fn index(&self, index: T) -> &Self::Output { &self.deref()[index] }
}
impl Borrow<[Tok<String>]> for Sym {
	fn borrow(&self) -> &[Tok<String>] { &self.0[..] }
}
impl Borrow<PathSlice> for Sym {
	fn borrow(&self) -> &PathSlice { PathSlice::new(&self.0[..]) }
}
impl Deref for Sym {
	type Target = PathSlice;
	fn deref(&self) -> &Self::Target { self.borrow() }
}

/// An abstraction over tokenized vs non-tokenized names so that they can be
/// handled together in datastructures. The names can never be empty
#[allow(clippy::len_without_is_empty)] // never empty
pub trait NameLike:
	'static + Clone + Eq + Hash + fmt::Debug + fmt::Display + Borrow<PathSlice>
{
	/// Convert into held slice
	fn as_slice(&self) -> &[Tok<String>] { Borrow::<PathSlice>::borrow(self) }
	/// Get iterator over tokens
	fn iter(&self) -> impl NameIter + '_ { self.as_slice().iter().cloned() }
	/// Get iterator over string segments
	fn str_iter(&self) -> impl Iterator<Item = &'_ str> + '_ {
		self.as_slice().iter().map(|t| t.as_str())
	}
	/// Fully resolve the name for printing
	#[must_use]
	fn to_strv(&self) -> Vec<String> { self.iter().map(|s| s.to_string()).collect() }
	/// Format the name as an approximate filename
	fn as_src_path(&self) -> String { format!("{}.orc", self.iter().join("/")) }
	/// Return the number of segments in the name
	fn len(&self) -> NonZeroUsize {
		NonZeroUsize::try_from(self.iter().count()).expect("NameLike never empty")
	}
	/// Like slice's `split_first` except we know that it always returns Some
	fn split_first(&self) -> (Tok<String>, &PathSlice) {
		let (foot, torso) = self.as_slice().split_last().expect("NameLike never empty");
		(foot.clone(), PathSlice::new(torso))
	}
	/// Like slice's `split_last` except we know that it always returns Some
	fn split_last(&self) -> (Tok<String>, &PathSlice) {
		let (foot, torso) = self.as_slice().split_last().expect("NameLike never empty");
		(foot.clone(), PathSlice::new(torso))
	}
	/// Get the first element
	fn first(&self) -> Tok<String> { self.split_first().0 }
	/// Get the last element
	fn last(&self) -> Tok<String> { self.split_last().0 }
}

impl NameLike for Sym {}
impl NameLike for VName {}

/// Create a [Sym] literal.
///
/// Both the name and its components will be cached in a thread-local static so
/// that subsequent executions of the expression only incur an Arc-clone for
/// cloning the token.
#[macro_export]
macro_rules! sym {
  ($seg1:tt $( :: $seg:tt)* ; $i:expr) => { async {
		$crate::name::Sym::from_tok(
			$i.i(&[
				$i.i(stringify!($seg1)).await
				$( , $i.i(stringify!($seg)).await )*
			])
			.await
		).unwrap()
		}
  };
  (@NAME $seg:tt) => {}
}

/// Create a [VName] literal.
///
/// The components are interned much like in [sym].
#[macro_export]
macro_rules! vname {
  ($seg1:tt $( :: $seg:tt)* ; $i:expr) => { async {
    $crate::name::VName::new([
      $i.i(stringify!($seg1)).await
      $( , $i.i(stringify!($seg)).await )*
    ]).unwrap()
	} };
}

/// Create a [VPath] literal.
///
/// The components are interned much like in [sym].
#[macro_export]
macro_rules! vpath {
  ($seg1:tt $( :: $seg:tt)+ ; $i:expr) => { async {
    $crate::name::VPath(vec![
      $i.i(stringify!($seg1)).await
      $( , $i.i(stringify!($seg)).await )+
		])
	} };
  () => {
    $crate::name::VPath(vec![])
  }
}

#[cfg(test)]
mod test {
	use std::borrow::Borrow;

	use test_executors::spin_on;

	use super::{PathSlice, Sym, VName};
	use crate::interner::{Interner, Tok};
	use crate::name::VPath;

	#[test]
	fn recur() {
		spin_on(async {
			let i = Interner::new_master();
			let myname = vname!(foo::bar; i).await;
			let _borrowed_slice: &[Tok<String>] = myname.borrow();
			let _borrowed_pathslice: &PathSlice = myname.borrow();
			let _deref_pathslice: &PathSlice = &myname;
			let _as_slice_out: &[Tok<String>] = myname.as_slice();
		})
	}

	#[test]
	fn literals() {
		spin_on(async {
			let i = Interner::new_master();
			assert_eq!(
				sym!(foo::bar::baz; i).await,
				Sym::new([i.i("foo").await, i.i("bar").await, i.i("baz").await], &i).await.unwrap()
			);
			assert_eq!(
				vname!(foo::bar::baz; i).await,
				VName::new([i.i("foo").await, i.i("bar").await, i.i("baz").await]).unwrap()
			);
			assert_eq!(
				vpath!(foo::bar::baz; i).await,
				VPath::new([i.i("foo").await, i.i("bar").await, i.i("baz").await])
			);
		})
	}
}
