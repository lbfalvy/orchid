use std::any::{Any, TypeId, type_name};
use std::fmt;
use std::future::Future;
use std::num::NonZero;
use std::pin::Pin;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use memo_map::MemoMap;
use orchid_api::ExtMsgSet;
use orchid_api_traits::{Coding, Decode};
use orchid_base::boxed_iter::BoxedIter;
use orchid_base::builtin::Spawner;
use orchid_base::interner::Interner;
use orchid_base::logging::Logger;
use orchid_base::reqnot::{Receipt, ReqNot};

use crate::api;
use crate::atom::{AtomCtx, AtomDynfo, AtomTypeId, AtomicFeatures, ForeignAtom, TypAtom, get_info};
use crate::entrypoint::ExtReq;
use crate::fs::DeclFs;
use crate::func_atom::Fun;
use crate::lexer::LexerObj;
use crate::parser::ParserObj;
use crate::system_ctor::{CtedObj, SystemCtor};
use crate::tree::GenItem;

/// System as consumed by foreign code
pub trait SystemCard: Default + Send + Sync + 'static {
	type Ctor: SystemCtor;
	type Req: Coding;
	fn atoms() -> impl IntoIterator<Item = Option<Box<dyn AtomDynfo>>>;
}

pub trait DynSystemCard: Send + Sync + 'static {
	fn name(&self) -> &'static str;
	/// Atoms explicitly defined by the system card. Do not rely on this for
	/// querying atoms as it doesn't include the general atom types
	fn atoms(&self) -> BoxedIter<Option<Box<dyn AtomDynfo>>>;
}

/// Atoms supported by this package which may appear in all extensions.
/// The indices of these are bitwise negated, such that the MSB of an atom index
/// marks whether it belongs to this package (0) or the importer (1)
fn general_atoms() -> impl Iterator<Item = Option<Box<dyn AtomDynfo>>> {
	[Some(Fun::dynfo())].into_iter()
}

pub fn atom_info_for(
	sys: &(impl DynSystemCard + ?Sized),
	tid: TypeId,
) -> Option<(AtomTypeId, Box<dyn AtomDynfo>)> {
	(sys.atoms().enumerate().map(|(i, o)| (NonZero::new(i as u32 + 1).unwrap(), o)))
		.chain(general_atoms().enumerate().map(|(i, o)| (NonZero::new(!(i as u32)).unwrap(), o)))
		.filter_map(|(i, o)| o.map(|a| (AtomTypeId(i), a)))
		.find(|ent| ent.1.tid() == tid)
}

pub fn atom_by_idx(
	sys: &(impl DynSystemCard + ?Sized),
	tid: AtomTypeId,
) -> Option<Box<dyn AtomDynfo>> {
	if (u32::from(tid.0) >> (u32::BITS - 1)) & 1 == 1 {
		general_atoms().nth(!u32::from(tid.0) as usize).unwrap()
	} else {
		sys.atoms().nth(u32::from(tid.0) as usize - 1).unwrap()
	}
}

pub async fn resolv_atom(
	sys: &(impl DynSystemCard + ?Sized),
	atom: &api::Atom,
) -> Box<dyn AtomDynfo> {
	let tid = AtomTypeId::decode(Pin::new(&mut &atom.data[..])).await;
	atom_by_idx(sys, tid).expect("Value of nonexistent type found")
}

impl<T: SystemCard> DynSystemCard for T {
	fn name(&self) -> &'static str { T::Ctor::NAME }
	fn atoms(&self) -> BoxedIter<Option<Box<dyn AtomDynfo>>> { Box::new(Self::atoms().into_iter()) }
}

/// System as defined by author
pub trait System: Send + Sync + SystemCard + 'static {
	fn env() -> Vec<GenItem>;
	fn vfs() -> DeclFs;
	fn lexers() -> Vec<LexerObj>;
	fn parsers() -> Vec<ParserObj>;
	fn request(hand: ExtReq<'_>, req: Self::Req) -> impl Future<Output = Receipt<'_>>;
}

pub trait DynSystem: Send + Sync + DynSystemCard + 'static {
	fn dyn_env(&self) -> Vec<GenItem>;
	fn dyn_vfs(&self) -> DeclFs;
	fn dyn_lexers(&self) -> Vec<LexerObj>;
	fn dyn_parsers(&self) -> Vec<ParserObj>;
	fn dyn_request<'a>(&self, hand: ExtReq<'a>, req: Vec<u8>) -> LocalBoxFuture<'a, Receipt<'a>>;
	fn card(&self) -> &dyn DynSystemCard;
}

impl<T: System> DynSystem for T {
	fn dyn_env(&self) -> Vec<GenItem> { Self::env() }
	fn dyn_vfs(&self) -> DeclFs { Self::vfs() }
	fn dyn_lexers(&self) -> Vec<LexerObj> { Self::lexers() }
	fn dyn_parsers(&self) -> Vec<ParserObj> { Self::parsers() }
	fn dyn_request<'a>(&self, hand: ExtReq<'a>, req: Vec<u8>) -> LocalBoxFuture<'a, Receipt<'a>> {
		Box::pin(async move {
			Self::request(hand, <Self as SystemCard>::Req::decode(Pin::new(&mut &req[..])).await).await
		})
	}
	fn card(&self) -> &dyn DynSystemCard { self }
}

pub async fn downcast_atom<A>(foreign: ForeignAtom) -> Result<TypAtom<A>, ForeignAtom>
where A: AtomicFeatures {
	let mut data = &foreign.atom.data[..];
	let ctx = foreign.ctx().clone();
	let value = AtomTypeId::decode(Pin::new(&mut data)).await;
	let own_inst = ctx.get::<CtedObj>().inst();
	let owner = if *ctx.get::<api::SysId>() == foreign.atom.owner {
		own_inst.card()
	} else {
		(ctx.get::<CtedObj>().deps().find(|s| s.id() == foreign.atom.owner))
			.ok_or_else(|| foreign.clone())?
			.get_card()
	};
	let (typ_id, dynfo) = get_info::<A>(owner);
	if value != typ_id {
		return Err(foreign);
	}
	let val = dynfo.decode(AtomCtx(data, foreign.atom.drop, ctx)).await;
	let value = *val.downcast::<A::Data>().expect("atom decode returned wrong type");
	Ok(TypAtom { value, data: foreign })
}

// #[derive(Clone)]
// pub struct SysCtx {
// 	pub reqnot: ReqNot<api::ExtMsgSet>,
// 	pub spawner: Spawner,
// 	pub id: api::SysId,
// 	pub cted: CtedObj,
// 	pub logger: Logger,
// 	pub obj_store: ObjStore,
// 	pub i: Rc<Interner>,
// }
// impl fmt::Debug for SysCtx {
// 	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
// 		write!(f, "SysCtx({:?})", self.id)
// 	}
// }

#[derive(Clone)]
pub struct SysCtx(Rc<MemoMap<TypeId, Box<dyn Any>>>);
impl SysCtx {
	pub fn new(
		id: api::SysId,
		i: Interner,
		reqnot: ReqNot<ExtMsgSet>,
		spawner: Spawner,
		logger: Logger,
		cted: CtedObj,
	) -> Self {
		let this = Self(Rc::new(MemoMap::new()));
		this.add(id).add(i).add(reqnot).add(spawner).add(logger).add(cted);
		this
	}
	pub fn add<T: SysCtxEntry>(&self, t: T) -> &Self {
		assert!(self.0.insert(TypeId::of::<T>(), Box::new(t)), "Key already exists");
		self
	}
	pub fn get_or_insert<T: SysCtxEntry>(&self, f: impl FnOnce() -> T) -> &T {
		(self.0.get_or_insert_owned(TypeId::of::<T>(), || Box::new(f())).downcast_ref())
			.expect("Keyed by TypeId")
	}
	pub fn get_or_default<T: SysCtxEntry + Default>(&self) -> &T {
		self.get_or_insert(|| {
			let rc_id = self.0.as_ref() as *const _ as *const () as usize;
			eprintln!("Default-initializing {} in {}", type_name::<T>(), rc_id);
			T::default()
		})
	}
	pub fn try_get<T: SysCtxEntry>(&self) -> Option<&T> {
		Some(self.0.get(&TypeId::of::<T>())?.downcast_ref().expect("Keyed by TypeId"))
	}
	pub fn get<T: SysCtxEntry>(&self) -> &T {
		self.try_get().unwrap_or_else(|| panic!("Context {} missing", type_name::<T>()))
	}
	/// Shorthand to get the [Interner] instance
	pub fn i(&self) -> &Interner { self.get::<Interner>() }
	/// Shorthand to get the messaging link
	pub fn reqnot(&self) -> &ReqNot<ExtMsgSet> { self.get::<ReqNot<ExtMsgSet>>() }
	/// Shorthand to get the system ID
	pub fn sys_id(&self) -> api::SysId { *self.get::<api::SysId>() }
	/// Shorthand to get the task spawner callback
	pub fn spawner(&self) -> &Spawner { self.get::<Spawner>() }
	/// Shorthand to get the logger
	pub fn logger(&self) -> &Logger { self.get::<Logger>() }
	/// Shorthand to get the constructed system object
	pub fn cted(&self) -> &CtedObj { self.get::<CtedObj>() }
}
impl fmt::Debug for SysCtx {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "SysCtx({:?})", self.sys_id())
	}
}
pub trait SysCtxEntry: 'static + Sized {}
impl SysCtxEntry for api::SysId {}
impl SysCtxEntry for ReqNot<api::ExtMsgSet> {}
impl SysCtxEntry for Spawner {}
impl SysCtxEntry for CtedObj {}
impl SysCtxEntry for Logger {}
impl SysCtxEntry for Interner {}
