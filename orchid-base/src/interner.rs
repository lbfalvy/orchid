use std::borrow::Borrow;
use std::hash::BuildHasher as _;
use std::num::NonZeroU64;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard, atomic};
use std::{fmt, hash, mem};

use hashbrown::{HashMap, HashSet};
use itertools::Itertools as _;
use orchid_api_traits::{Decode, Encode, Request};

use crate::api;
use crate::reqnot::{DynRequester, Requester};

/// Clippy crashes while verifying `Tok: Sized` without this and I cba to create
/// a minimal example
#[derive(Clone)]
struct ForceSized<T>(T);

#[derive(Clone)]
pub struct Tok<T: Interned> {
	data: Arc<T>,
	marker: ForceSized<T::Marker>,
}
impl<T: Interned> Tok<T> {
	pub fn new(data: Arc<T>, marker: T::Marker) -> Self { Self { data, marker: ForceSized(marker) } }
	pub fn to_api(&self) -> T::Marker { self.marker.0 }
	pub fn from_api<M>(marker: M) -> Self
	where M: InternMarker<Interned = T> {
		deintern(marker)
	}
	pub fn arc(&self) -> Arc<T> { self.data.clone() }
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
impl<T: Interned + Encode> Encode for Tok<T> {
	fn encode<W: std::io::Write + ?Sized>(&self, write: &mut W) { self.data.encode(write) }
}
impl<T: Interned + Decode> Decode for Tok<T> {
	fn decode<R: std::io::Read + ?Sized>(read: &mut R) -> Self { intern(&T::decode(read)) }
}

pub trait Interned: Eq + hash::Hash + Clone + fmt::Debug + Internable<Interned = Self> {
	type Marker: InternMarker<Interned = Self> + Sized;
	fn intern(
		self: Arc<Self>,
		req: &(impl DynRequester<Transfer = api::IntReq> + ?Sized),
	) -> Self::Marker;
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
	) -> Tok<Self::Interned>;
	fn get_id(self) -> NonZeroU64;
	fn from_id(id: NonZeroU64) -> Self;
}

impl Interned for String {
	type Marker = api::TStr;
	fn intern(
		self: Arc<Self>,
		req: &(impl DynRequester<Transfer = api::IntReq> + ?Sized),
	) -> Self::Marker {
		req.request(api::InternStr(self))
	}
	fn bimap(interners: &mut TypedInterners) -> &mut Bimap<Self> { &mut interners.strings }
}
impl InternMarker for api::TStr {
	type Interned = String;
	fn resolve(
		self,
		req: &(impl DynRequester<Transfer = api::IntReq> + ?Sized),
	) -> Tok<Self::Interned> {
		Tok::new(req.request(api::ExternStr(self)), self)
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
	fn intern(
		self: Arc<Self>,
		req: &(impl DynRequester<Transfer = api::IntReq> + ?Sized),
	) -> Self::Marker {
		req.request(api::InternStrv(Arc::new(self.iter().map(|t| t.to_api()).collect())))
	}
	fn bimap(interners: &mut TypedInterners) -> &mut Bimap<Self> { &mut interners.vecs }
}
impl InternMarker for api::TStrv {
	type Interned = Vec<Tok<String>>;
	fn resolve(
		self,
		req: &(impl DynRequester<Transfer = api::IntReq> + ?Sized),
	) -> Tok<Self::Interned> {
		let data =
			Arc::new(req.request(api::ExternStrv(self)).iter().map(|m| deintern(*m)).collect_vec());
		Tok::new(data, self)
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
impl Internable for Vec<api::TStr> {
	type Interned = Vec<Tok<String>>;
	fn get_owned(&self) -> Arc<Self::Interned> {
		Arc::new(self.iter().map(|ts| deintern(*ts)).collect())
	}
}
impl Internable for [api::TStr] {
	type Interned = Vec<Tok<String>>;
	fn get_owned(&self) -> Arc<Self::Interned> {
		Arc::new(self.iter().map(|ts| deintern(*ts)).collect())
	}
}

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

static ID: atomic::AtomicU64 = atomic::AtomicU64::new(1);
static INTERNER: Mutex<Option<Interner>> = Mutex::new(None);

pub fn interner() -> impl DerefMut<Target = Interner> {
	struct G(MutexGuard<'static, Option<Interner>>);
	impl Deref for G {
		type Target = Interner;
		fn deref(&self) -> &Self::Target { self.0.as_ref().expect("Guard pre-initialized") }
	}
	impl DerefMut for G {
		fn deref_mut(&mut self) -> &mut Self::Target {
			self.0.as_mut().expect("Guard pre-iniitialized")
		}
	}
	let mut g = INTERNER.lock().unwrap();
	g.get_or_insert_with(Interner::default);
	G(g)
}

/// Initialize the interner in replica mode. No messages are sent at this point.
pub fn init_replica(req: impl DynRequester<Transfer = api::IntReq> + 'static) {
	let mut g = INTERNER.lock().unwrap();
	assert!(g.is_none(), "Attempted to initialize replica interner after first use");
	*g = Some(Interner {
		master: Some(Box::new(req)),
		interners: TypedInterners { strings: Bimap::default(), vecs: Bimap::default() },
	})
}

pub fn intern<T: Interned>(t: &(impl Internable<Interned = T> + ?Sized)) -> Tok<T> {
	let data = t.get_owned();
	let mut g = interner();
	let job = format!("{t:?} in {}", if g.master.is_some() { "replica" } else { "master" });
	eprintln!("Interning {job}");
	let typed = T::bimap(&mut g.interners);
	if let Some(tok) = typed.by_value(&data) {
		return tok;
	}
	let marker = match &mut g.master {
		Some(c) => data.clone().intern(&**c),
		None =>
			T::Marker::from_id(NonZeroU64::new(ID.fetch_add(1, atomic::Ordering::Relaxed)).unwrap()),
	};
	let tok = Tok::new(data, marker);
	T::bimap(&mut g.interners).insert(tok.clone());
	mem::drop(g);
	eprintln!("Interned {job}");
	tok
}

fn deintern<M: InternMarker>(marker: M) -> Tok<M::Interned> {
	let mut g = interner();
	if let Some(tok) = M::Interned::bimap(&mut g.interners).by_marker(marker) {
		return tok;
	}
	let master = g.master.as_mut().expect("ID not in local interner and this is master");
	let token = marker.resolve(&**master);
	M::Interned::bimap(&mut g.interners).insert(token.clone());
	token
}

pub fn merge_retained(into: &mut api::Retained, from: &api::Retained) {
	into.strings = into.strings.iter().chain(&from.strings).copied().unique().collect();
	into.vecs = into.vecs.iter().chain(&from.vecs).copied().unique().collect();
}

pub fn sweep_replica() -> api::Retained {
	let mut g = interner();
	assert!(g.master.is_some(), "Not a replica");
	api::Retained {
		strings: g.interners.strings.sweep_replica(),
		vecs: g.interners.vecs.sweep_replica(),
	}
}

/// Create a thread-local token instance and copy it. This ensures that the
/// interner will only be called the first time the expresion is executed,
/// and subsequent calls will just copy the token. Accepts a single static
/// expression (i.e. a literal).
#[macro_export]
macro_rules! intern {
	($ty:ty : $expr:expr) => {{
		thread_local! {
			static VALUE: $crate::interner::Tok<<$ty as $crate::interner::Internable>::Interned>
				= $crate::interner::intern::<
						<$ty as $crate::interner::Internable>::Interned
					>($expr as &$ty);
		}
		VALUE.with(|v| v.clone())
	}};
}

pub fn sweep_master(retained: api::Retained) {
	let mut g = interner();
	assert!(g.master.is_none(), "Not master");
	g.interners.strings.sweep_master(retained.strings.into_iter().collect());
	g.interners.vecs.sweep_master(retained.vecs.into_iter().collect());
}

#[cfg(test)]
mod test {
	use std::num::NonZero;

	use orchid_api_traits::{Decode, enc_vec};

	use super::*;
	use crate::api;

	#[test]
	fn test_i() {
		let _: Tok<String> = intern!(str: "foo");
		let _: Tok<Vec<Tok<String>>> = intern!([Tok<String>]: &[
			intern!(str: "bar"),
			intern!(str: "baz")
		]);
	}

	#[test]
	fn test_coding() {
		let coded = api::TStr(NonZero::new(3u64).unwrap());
		let mut enc = &enc_vec(&coded)[..];
		api::TStr::decode(&mut enc);
		assert_eq!(enc, [], "Did not consume all of {enc:?}")
	}
}
