use std::any::TypeId;
use std::future::Future;
use std::num::NonZero;
use std::sync::Arc;

use futures::FutureExt;
use futures::future::LocalBoxFuture;
use futures::task::LocalSpawn;
use hashbrown::HashMap;
use orchid_api_traits::{Coding, Decode};
use orchid_base::boxed_iter::BoxedIter;
use orchid_base::id_store::IdStore;
use orchid_base::interner::Tok;
use orchid_base::logging::Logger;
use orchid_base::reqnot::{Receipt, ReqNot};

use crate::api;
use crate::atom::{AtomCtx, AtomDynfo, AtomicFeatures, ForeignAtom, TypAtom, get_info};
use crate::atom_owned::{DynOwnedAtom, ObjStore};
use crate::entrypoint::ExtReq;
use crate::fs::DeclFs;
use crate::func_atom::Fun;
use crate::lexer::LexerObj;
use crate::parser::ParserObj;
use crate::system_ctor::{CtedObj, SystemCtor};
use crate::tree::MemKind;

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
) -> Option<(api::AtomId, Box<dyn AtomDynfo>)> {
	(sys.atoms().enumerate().map(|(i, o)| (NonZero::new(i as u64 + 1).unwrap(), o)))
		.chain(general_atoms().enumerate().map(|(i, o)| (NonZero::new(!(i as u64)).unwrap(), o)))
		.filter_map(|(i, o)| o.map(|a| (api::AtomId(i), a)))
		.find(|ent| ent.1.tid() == tid)
}

pub fn atom_by_idx(
	sys: &(impl DynSystemCard + ?Sized),
	tid: api::AtomId,
) -> Option<Box<dyn AtomDynfo>> {
	if (u64::from(tid.0) >> (u64::BITS - 1)) & 1 == 1 {
		general_atoms().nth(!u64::from(tid.0) as usize).unwrap()
	} else {
		sys.atoms().nth(u64::from(tid.0) as usize - 1).unwrap()
	}
}

pub fn resolv_atom(sys: &(impl DynSystemCard + ?Sized), atom: &api::Atom) -> Box<dyn AtomDynfo> {
	let tid = api::AtomId::decode(&mut &atom.data[..8]);
	atom_by_idx(sys, tid).expect("Value of nonexistent type found")
}

impl<T: SystemCard> DynSystemCard for T {
	fn name(&self) -> &'static str { T::Ctor::NAME }
	fn atoms(&self) -> BoxedIter<Option<Box<dyn AtomDynfo>>> { Box::new(Self::atoms().into_iter()) }
}

/// System as defined by author
pub trait System: Send + Sync + SystemCard + 'static {
	fn env() -> Vec<(Tok<String>, MemKind)>;
	fn vfs() -> DeclFs;
	fn lexers() -> Vec<LexerObj>;
	fn parsers() -> Vec<ParserObj>;
	fn request<'a>(hand: ExtReq<'a>, req: Self::Req) -> impl Future<Output = Receipt<'a>>;
}

pub trait DynSystem: Send + Sync + DynSystemCard + 'static {
	fn dyn_env(&self) -> HashMap<Tok<String>, MemKind>;
	fn dyn_vfs(&self) -> DeclFs;
	fn dyn_lexers(&self) -> Vec<LexerObj>;
	fn dyn_parsers(&self) -> Vec<ParserObj>;
	fn dyn_request<'a>(&'a self, hand: ExtReq<'a>, req: Vec<u8>) -> LocalBoxFuture<'a, Receipt<'a>>;
	fn card(&self) -> &dyn DynSystemCard;
}

impl<T: System> DynSystem for T {
	fn dyn_env(&self) -> HashMap<Tok<String>, MemKind> { Self::env().into_iter().collect() }
	fn dyn_vfs(&self) -> DeclFs { Self::vfs() }
	fn dyn_lexers(&self) -> Vec<LexerObj> { Self::lexers() }
	fn dyn_parsers(&self) -> Vec<ParserObj> { Self::parsers() }
	fn dyn_request<'a>(&'a self, hand: ExtReq<'a>, req: Vec<u8>) -> LocalBoxFuture<'a, Receipt<'a>> {
		Self::request(hand, <Self as SystemCard>::Req::decode(&mut &req[..])).boxed_local()
	}
	fn card(&self) -> &dyn DynSystemCard { self }
}

pub fn downcast_atom<A: AtomicFeatures>(foreign: ForeignAtom) -> Result<TypAtom<A>, ForeignAtom> {
	let mut data = &foreign.atom.data[..];
	let ctx = foreign.ctx.clone();
	let info_ent = (ctx.cted.deps().find(|s| s.id() == foreign.atom.owner))
		.map(|sys| get_info::<A>(sys.get_card()))
		.filter(|(pos, _)| api::AtomId::decode(&mut data) == *pos);
	match info_ent {
		None => Err(foreign),
		Some((_, info)) => {
			let val = info.decode(AtomCtx(data, foreign.atom.drop, ctx));
			let value = *val.downcast::<A::Data>().expect("atom decode returned wrong type");
			Ok(TypAtom { value, data: foreign })
		},
	}
}

#[derive(Clone)]
pub struct SysCtx {
	pub reqnot: ReqNot<api::ExtMsgSet>,
	pub spawner: Arc<dyn LocalSpawn>,
	pub id: api::SysId,
	pub cted: CtedObj,
	pub logger: Arc<Logger>,
	pub obj_store: ObjStore,
}
