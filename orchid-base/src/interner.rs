use std::borrow::Borrow;
use std::hash::BuildHasher as _;
use std::num::NonZeroU64;
use std::ops::{Deref, DerefMut};
use std::sync::{atomic, Arc, Mutex, MutexGuard};
use std::{fmt, hash};

use hashbrown::{HashMap, HashSet};
use itertools::Itertools as _;
use orchid_api::interner::{
  ExternStr, ExternStrv, IntReq, InternStr, InternStrv, Retained, TStr, TStrv,
};
use orchid_api_traits::Request;

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
  pub fn marker(&self) -> T::Marker { self.marker.0 }
  pub fn arc(&self) -> Arc<T> { self.data.clone() }
}
impl<T: Interned> Deref for Tok<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target { self.data.as_ref() }
}
impl<T: Interned> Ord for Tok<T> {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering { self.marker().cmp(&other.marker()) }
}
impl<T: Interned> PartialOrd for Tok<T> {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}
impl<T: Interned> Eq for Tok<T> {}
impl<T: Interned> PartialEq for Tok<T> {
  fn eq(&self, other: &Self) -> bool { self.cmp(other).is_eq() }
}
impl<T: Interned> hash::Hash for Tok<T> {
  fn hash<H: hash::Hasher>(&self, state: &mut H) { self.marker().hash(state) }
}
impl<T: Interned + fmt::Display> fmt::Display for Tok<T> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", &*self.data)
  }
}
impl<T: Interned + fmt::Debug> fmt::Debug for Tok<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Token({} -> {:?})", self.marker().get_id(), self.data.as_ref())
  }
}

pub trait Interned: Eq + hash::Hash + Clone {
  type Marker: InternMarker<Interned = Self> + Sized;
  fn intern(self: Arc<Self>, req: &(impl DynRequester<Transfer = IntReq> + ?Sized))
  -> Self::Marker;
  fn bimap(interner: &mut TypedInterners) -> &mut Bimap<Self>;
}

pub trait Internable {
  type Interned: Interned;
  fn get_owned(&self) -> Arc<Self::Interned>;
}

pub trait InternMarker: Copy + PartialEq + Eq + PartialOrd + Ord + hash::Hash + Sized {
  type Interned: Interned<Marker = Self>;
  fn resolve(self, req: &(impl DynRequester<Transfer = IntReq> + ?Sized)) -> Tok<Self::Interned>;
  fn get_id(self) -> NonZeroU64;
  fn from_id(id: NonZeroU64) -> Self;
}

impl Interned for String {
  type Marker = TStr;
  fn intern(
    self: Arc<Self>,
    req: &(impl DynRequester<Transfer = IntReq> + ?Sized),
  ) -> Self::Marker {
    req.request(InternStr(self))
  }
  fn bimap(interners: &mut TypedInterners) -> &mut Bimap<Self> { &mut interners.strings }
}

impl InternMarker for TStr {
  type Interned = String;
  fn resolve(self, req: &(impl DynRequester<Transfer = IntReq> + ?Sized)) -> Tok<Self::Interned> {
    Tok::new(req.request(ExternStr(self)), self)
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
  type Marker = TStrv;
  fn intern(
    self: Arc<Self>,
    req: &(impl DynRequester<Transfer = IntReq> + ?Sized),
  ) -> Self::Marker {
    req.request(InternStrv(Arc::new(self.iter().map(|t| t.marker()).collect())))
  }
  fn bimap(interners: &mut TypedInterners) -> &mut Bimap<Self> { &mut interners.vecs }
}

impl InternMarker for TStrv {
  type Interned = Vec<Tok<String>>;
  fn resolve(self, req: &(impl DynRequester<Transfer = IntReq> + ?Sized)) -> Tok<Self::Interned> {
    let data = Arc::new(req.request(ExternStrv(self)).iter().map(|m| deintern(*m)).collect_vec());
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

impl Internable for Vec<TStr> {
  type Interned = Vec<Tok<String>>;
  fn get_owned(&self) -> Arc<Self::Interned> {
    Arc::new(self.iter().map(|ts| deintern(*ts)).collect())
  }
}

impl Internable for [TStr] {
  type Interned = Vec<Tok<String>>;
  fn get_owned(&self) -> Arc<Self::Interned> {
    Arc::new(self.iter().map(|ts| deintern(*ts)).collect())
  }
}

/// The number of references held to any token by the interner.
const BASE_RC: usize = 3;

#[test]
fn base_rc_correct() {
  let tok = Tok::new(Arc::new("foo".to_string()), TStr(1.try_into().unwrap()));
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
    self.by_id.insert(token.marker(), token);
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
        self.by_id.remove(&v.marker());
        v.marker()
      })
      .collect()
  }

  pub fn sweep_master(&mut self, retained: HashSet<T::Marker>) {
    self.intern.retain(|k, v| BASE_RC < Arc::strong_count(k) || retained.contains(&v.marker()))
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
  master: Option<Box<dyn DynRequester<Transfer = IntReq>>>,
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

pub fn init_replica(req: impl DynRequester<Transfer = IntReq> + 'static) {
  let mut g = INTERNER.lock().unwrap();
  assert!(g.is_none(), "Attempted to initialize replica interner after first use");
  *g = Some(Interner {
    master: Some(Box::new(req)),
    interners: TypedInterners { strings: Bimap::default(), vecs: Bimap::default() },
  })
}

pub fn intern<T: Interned>(t: &(impl Internable<Interned = T> + ?Sized)) -> Tok<T> {
  let mut g = interner();
  let data = t.get_owned();
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
  tok
}

pub fn deintern<M: InternMarker>(marker: M) -> Tok<M::Interned> {
  let mut g = interner();
  if let Some(tok) = M::Interned::bimap(&mut g.interners).by_marker(marker) {
    return tok;
  }
  let master = g.master.as_mut().expect("ID not in local interner and this is master");
  let token = marker.resolve(&**master);
  M::Interned::bimap(&mut g.interners).insert(token.clone());
  token
}

pub fn merge_retained(into: &mut Retained, from: &Retained) {
  into.strings = into.strings.iter().chain(&from.strings).copied().unique().collect();
  into.vecs = into.vecs.iter().chain(&from.vecs).copied().unique().collect();
}

pub fn sweep_replica() -> Retained {
  let mut g = interner();
  assert!(g.master.is_some(), "Not a replica");
  Retained { strings: g.interners.strings.sweep_replica(), vecs: g.interners.vecs.sweep_replica() }
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

pub fn sweep_master(retained: Retained) {
  eprintln!(
    "Hello, {:?}",
    intern!([Tok<String>]: &[
      intern!(str: "bar"),
      intern!(str: "baz")
    ])
  );
  let mut g = interner();
  assert!(g.master.is_none(), "Not master");
  g.interners.strings.sweep_master(retained.strings.into_iter().collect());
  g.interners.vecs.sweep_master(retained.vecs.into_iter().collect());
}

#[test]
fn test_i() {
  let _: Tok<String> = intern!(str: "foo");
  let _: Tok<Vec<Tok<String>>> = intern!([Tok<String>]: &[
    intern!(str: "bar"),
    intern!(str: "baz")
  ]);
}