use std::borrow::Borrow;
use std::future::Future;
use std::hash::BuildHasher as _;
use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard, atomic};
use std::{fmt, hash, mem};

use async_once_cell::Lazy;
use hashbrown::{HashMap, HashSet};
use itertools::Itertools as _;
use orchid_api_traits::Request;

use crate::api;
use crate::reqnot::{DynRequester, Requester};

/// Clippy crashes while verifying `Tok: Sized` without this and I cba to create
/// a minimal example
#[derive(Clone)]
struct ForceSized<T>(T);

#[derive(Clone)]
pub struct Tok<T: Interned> {
	data: Rc<T>,
	marker: ForceSized<T::Marker>,
}
impl<T: Interned> Tok<T> {
	pub fn new(data: Rc<T>, marker: T::Marker) -> Self { Self { data, marker: ForceSized(marker) } }
	pub fn to_api(&self) -> T::Marker { self.marker.0 }
	pub async fn from_api<M>(marker: M, i: &mut Interner) -> Self
	where M: InternMarker<Interned = T> {
		i.deintern(marker).await
	}
	pub fn rc(&self) -> Rc<T> { self.data.clone() }
}
impl<T: Interned> Deref for Tok<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target { self.data.as_ref() }
}
impl<T: Interned> Ord for Tok<T> {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering { self.to_api().cmp(&other.to_api()) }
}
impl<T: Interned> PartialOrd for Tok<T> {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}
impl<T: Interned> Eq for Tok<T> {}
impl<T: Interned> PartialEq for Tok<T> {
	fn eq(&self, other: &Self) -> bool { self.cmp(other).is_eq() }
}
impl<T: Interned> hash::Hash for Tok<T> {
	fn hash<H: hash::Hasher>(&self, state: &mut H) { self.to_api().hash(state) }
}
impl<T: Interned + fmt::Display> fmt::Display for Tok<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", &*self.data)
	}
}
impl<T: Interned + fmt::Debug> fmt::Debug for Tok<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Token({} -> {:?})", self.to_api().get_id(), self.data.as_ref())
	}
}

pub trait Interned: Eq + hash::Hash + Clone + fmt::Debug + Internable<Interned = Self> {
	type Marker: InternMarker<Interned = Self> + Sized;
	fn intern(
		self: Arc<Self>,
		req: &(impl DynRequester<Transfer = api::IntReq> + ?Sized),
	) -> impl Future<Output = Self::Marker>;
	fn bimap(interner: &mut TypedInterners) -> &mut Bimap<Self>;
}

pub trait Internable: fmt::Debug {
	type Interned: Interned;
	fn get_owned(&self) -> Arc<Self::Interned>;
}

pub trait InternMarker: Copy + PartialEq + Eq + PartialOrd + Ord + hash::Hash + Sized {
	type Interned: Interned<Marker = Self>;
	fn resolve(
		self,
		req: &(impl DynRequester<Transfer = api::IntReq> + ?Sized),
	) -> impl Future<Output = Tok<Self::Interned>>;
	fn get_id(self) -> NonZeroU64;
	fn from_id(id: NonZeroU64) -> Self;
}

impl Interned for String {
	type Marker = api::TStr;
	async fn intern(
		self: Arc<Self>,
		req: &(impl DynRequester<Transfer = api::IntReq> + ?Sized),
	) -> Self::Marker {
		req.request(api::InternStr(self)).await
	}
	fn bimap(interners: &mut TypedInterners) -> &mut Bimap<Self> { &mut interners.strings }
}
impl InternMarker for api::TStr {
	type Interned = String;
	async fn resolve(
		self,
		req: &(impl DynRequester<Transfer = api::IntReq> + ?Sized),
	) -> Tok<Self::Interned> {
		Tok::new(req.request(api::ExternStr(self)).await, self)
	}
	fn get_id(self) -> NonZeroU64 { self.0 }
	fn from_id(id: NonZeroU64) -> Self { Self(id) }
}
impl Internable for str {
	type Interned = String;
	fn get_owned(&self) -> Arc<Self::Interned> { Arc::new(self.to_string()) }
}
impl Internable for String {
	type Interned = String;
	fn get_owned(&self) -> Arc<Self::Interned> { Arc::new(self.to_string()) }
}

impl Interned for Vec<Tok<String>> {
	type Marker = api::TStrv;
	async fn intern(
		self: Arc<Self>,
		req: &(impl DynRequester<Transfer = api::IntReq> + ?Sized),
	) -> Self::Marker {
		req.request(api::InternStrv(Arc::new(self.iter().map(|t| t.to_api()).collect()))).await
	}
	fn bimap(interners: &mut TypedInterners) -> &mut Bimap<Self> { &mut interners.vecs }
}
impl InternMarker for api::TStrv {
	type Interned = Vec<Tok<String>>;
	async fn resolve(
		self,
		req: &(impl DynRequester<Transfer = api::IntReq> + ?Sized),
	) -> Tok<Self::Interned> {
		let rep = req.request(api::ExternStrv(self)).await;
		let data = futures::future::join_all(rep.iter().map(|m| deintern(*m))).await;
		Tok::new(Arc::new(data), self)
	}
	fn get_id(self) -> NonZeroU64 { self.0 }
	fn from_id(id: NonZeroU64) -> Self { Self(id) }
}
impl Internable for [Tok<String>] {
	type Interned = Vec<Tok<String>>;
	fn get_owned(&self) -> Arc<Self::Interned> { Arc::new(self.to_vec()) }
}
impl Internable for Vec<Tok<String>> {
	type Interned = Vec<Tok<String>>;
	fn get_owned(&self) -> Arc<Self::Interned> { Arc::new(self.to_vec()) }
}
// impl Internable for Vec<api::TStr> {
// 	type Interned = Vec<Tok<String>>;
// 	fn get_owned(&self) -> Arc<Self::Interned> {
// 		Arc::new(self.iter().map(|ts| deintern(*ts)).collect())
// 	}
// }
// impl Internable for [api::TStr] {
// 	type Interned = Vec<Tok<String>>;
// 	fn get_owned(&self) -> Arc<Self::Interned> {
// 		Arc::new(self.iter().map(|ts| deintern(*ts)).collect())
// 	}
// }

/// The number of references held to any token by the interner.
const BASE_RC: usize = 3;

#[test]
fn base_rc_correct() {
	let tok = Tok::new(Arc::new("foo".to_string()), api::TStr(1.try_into().unwrap()));
	let mut bimap = Bimap::default();
	bimap.insert(tok.clone());
	assert_eq!(Arc::strong_count(&tok.data), BASE_RC + 1, "the bimap plus the current instance");
}

pub struct Bimap<T: Interned> {
	intern: HashMap<Arc<T>, Tok<T>>,
	by_id: HashMap<T::Marker, Tok<T>>,
}
impl<T: Interned> Bimap<T> {
	pub fn insert(&mut self, token: Tok<T>) {
		self.intern.insert(token.data.clone(), token.clone());
		self.by_id.insert(token.to_api(), token);
	}

	pub fn by_marker(&self, marker: T::Marker) -> Option<Tok<T>> { self.by_id.get(&marker).cloned() }

	pub fn by_value<Q: Eq + hash::Hash>(&self, q: &Q) -> Option<Tok<T>>
	where T: Borrow<Q> {
		(self.intern.raw_entry())
			.from_hash(self.intern.hasher().hash_one(q), |k| k.as_ref().borrow() == q)
			.map(|p| p.1.clone())
	}

	pub fn sweep_replica(&mut self) -> Vec<T::Marker> {
		(self.intern)
			.extract_if(|k, _| Arc::strong_count(k) == BASE_RC)
			.map(|(_, v)| {
				self.by_id.remove(&v.to_api());
				v.to_api()
			})
			.collect()
	}

	pub fn sweep_master(&mut self, retained: HashSet<T::Marker>) {
		self.intern.retain(|k, v| BASE_RC < Arc::strong_count(k) || retained.contains(&v.to_api()))
	}
}

impl<T: Interned> Default for Bimap<T> {
	fn default() -> Self { Self { by_id: HashMap::new(), intern: HashMap::new() } }
}

pub trait UpComm {
	fn up<R: Request>(&self, req: R) -> R::Response;
}

#[derive(Default)]
pub struct TypedInterners {
	strings: Bimap<String>,
	vecs: Bimap<Vec<Tok<String>>>,
}

#[derive(Default)]
pub struct Interner {
	interners: TypedInterners,
	master: Option<Box<dyn DynRequester<Transfer = api::IntReq>>>,
}
impl Interner {
	pub fn new_master() -> Self { Self::default() }
	pub fn new_replica(req: impl DynRequester<Transfer = api::IntReq> + 'static) -> Self {
		Self {
			master: Some(Box::new(req)),
			interners: TypedInterners { strings: Bimap::default(), vecs: Bimap::default() },
		}
	}
	pub async fn intern<T: Interned>(
		&mut self,
		t: &(impl Internable<Interned = T> + ?Sized),
	) -> Tok<T> {
		let data = t.get_owned();
		let job = format!("{t:?} in {}", if self.master.is_some() { "replica" } else { "master" });
		eprintln!("Interning {job}");
		let typed = T::bimap(&mut self.interners);
		if let Some(tok) = typed.by_value(&data) {
			return tok;
		}
		let marker = match &mut self.master {
			Some(c) => data.clone().intern(&**c).await,
			None =>
				T::Marker::from_id(NonZeroU64::new(ID.fetch_add(1, atomic::Ordering::Relaxed)).unwrap()),
		};
		let tok = Tok::new(data, marker);
		T::bimap(&mut self.interners).insert(tok.clone());
		eprintln!("Interned {job}");
		tok
	}
	async fn deintern<M: InternMarker>(&mut self, marker: M) -> Tok<M::Interned> {
		if let Some(tok) = M::Interned::bimap(&mut self.interners).by_marker(marker) {
			return tok;
		}
		let master = self.master.as_mut().expect("ID not in local interner and this is master");
		let token = marker.resolve(&**master).await;
		M::Interned::bimap(&mut self.interners).insert(token.clone());
		token
	}
	pub fn sweep_replica(&mut self) -> api::Retained {
		assert!(self.master.is_some(), "Not a replica");
		api::Retained {
			strings: self.interners.strings.sweep_replica(),
			vecs: self.interners.vecs.sweep_replica(),
		}
	}
	pub fn sweep_master(&mut self, retained: api::Retained) {
		assert!(self.master.is_none(), "Not master");
		self.interners.strings.sweep_master(retained.strings.into_iter().collect());
		self.interners.vecs.sweep_master(retained.vecs.into_iter().collect());
	}
}

static ID: atomic::AtomicU64 = atomic::AtomicU64::new(1);

pub fn merge_retained(into: &mut api::Retained, from: &api::Retained) {
	into.strings = into.strings.iter().chain(&from.strings).copied().unique().collect();
	into.vecs = into.vecs.iter().chain(&from.vecs).copied().unique().collect();
}

#[cfg(test)]
mod test {
	use std::num::NonZero;

	use orchid_api_traits::{Decode, enc_vec};
	use test_executors::spin_on;

	use super::*;
	use crate::api;

	#[test]
	fn test_i() {
		let _: Tok<String> = spin_on(intern!(str: "foo"));
		let _: Tok<Vec<Tok<String>>> = spin_on(intern!([Tok<String>]: &[
			spin_on(intern!(str: "bar")),
			spin_on(intern!(str: "baz"))
		]));
	}

	#[test]
	fn test_coding() {
		let coded = api::TStr(NonZero::new(3u64).unwrap());
		let mut enc = &enc_vec(&coded)[..];
		api::TStr::decode(&mut enc);
		assert_eq!(enc, [], "Did not consume all of {enc:?}")
	}
}
