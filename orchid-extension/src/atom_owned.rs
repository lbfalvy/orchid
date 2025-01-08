use std::any::{Any, TypeId, type_name};
use std::borrow::Cow;
use std::io::{Read, Write};
use std::sync::Arc;

use itertools::Itertools;
use never::Never;
use orchid_api_traits::{Decode, Encode, enc_vec};
use orchid_base::error::OrcRes;
use orchid_base::id_store::{IdRecord, IdStore};
use orchid_base::name::Sym;

use crate::api;
use crate::atom::{
	AtomCard, AtomCtx, AtomDynfo, AtomFactory, Atomic, AtomicFeaturesImpl, AtomicVariant, MethodSet,
	err_not_callable, err_not_command, get_info,
};
use crate::expr::{Expr, ExprHandle, bot};
use crate::system::SysCtx;

pub struct OwnedVariant;
impl AtomicVariant for OwnedVariant {}
impl<A: OwnedAtom + Atomic<Variant = OwnedVariant>> AtomicFeaturesImpl<OwnedVariant> for A {
	fn _factory(self) -> AtomFactory {
		AtomFactory::new(move |ctx| {
			let rec = OBJ_STORE.add(Box::new(self));
			let (id, _) = get_info::<A>(ctx.cted.inst().card());
			let mut data = enc_vec(&id);
			rec.encode(&mut data);
			api::Atom { drop: Some(api::AtomId(rec.id())), data, owner: ctx.id }
		})
	}
	fn _info() -> Self::_Info { OwnedAtomDynfo(A::reg_reqs()) }
	type _Info = OwnedAtomDynfo<A>;
}

fn with_atom<U>(id: api::AtomId, f: impl FnOnce(IdRecord<'_, Box<dyn DynOwnedAtom>>) -> U) -> U {
	f(OBJ_STORE.get(id.0).unwrap_or_else(|| panic!("Received invalid atom ID: {}", id.0)))
}

pub struct OwnedAtomDynfo<T: OwnedAtom>(MethodSet<T>);
impl<T: OwnedAtom> AtomDynfo for OwnedAtomDynfo<T> {
	fn print(&self, AtomCtx(_, id, ctx): AtomCtx<'_>) -> String {
		with_atom(id.unwrap(), |a| a.dyn_print(ctx))
	}
	fn tid(&self) -> TypeId { TypeId::of::<T>() }
	fn name(&self) -> &'static str { type_name::<T>() }
	fn decode(&self, AtomCtx(data, ..): AtomCtx) -> Box<dyn Any> {
		Box::new(<T as AtomCard>::Data::decode(&mut &data[..]))
	}
	fn call(&self, AtomCtx(_, id, ctx): AtomCtx, arg: api::ExprTicket) -> Expr {
		with_atom(id.unwrap(), |a| a.remove().dyn_call(ctx, arg))
	}
	fn call_ref(&self, AtomCtx(_, id, ctx): AtomCtx, arg: api::ExprTicket) -> Expr {
		with_atom(id.unwrap(), |a| a.dyn_call_ref(ctx, arg))
	}
	fn handle_req(
		&self,
		AtomCtx(_, id, ctx): AtomCtx,
		key: Sym,
		req: &mut dyn Read,
		rep: &mut dyn Write,
	) -> bool {
		with_atom(id.unwrap(), |a| {
			self.0.dispatch(a.as_any_ref().downcast_ref().unwrap(), ctx, key, req, rep)
		})
	}
	fn command(&self, AtomCtx(_, id, ctx): AtomCtx<'_>) -> OrcRes<Option<Expr>> {
		with_atom(id.unwrap(), |a| a.remove().dyn_command(ctx))
	}
	fn drop(&self, AtomCtx(_, id, ctx): AtomCtx) {
		with_atom(id.unwrap(), |a| a.remove().dyn_free(ctx))
	}
	fn serialize(
		&self,
		AtomCtx(_, id, ctx): AtomCtx<'_>,
		write: &mut dyn Write,
	) -> Option<Vec<api::ExprTicket>> {
		let id = id.unwrap();
		id.encode(write);
		with_atom(id, |a| a.dyn_serialize(ctx, write))
			.map(|v| v.into_iter().map(|t| t.handle.unwrap().tk).collect_vec())
	}
	fn deserialize(&self, ctx: SysCtx, data: &[u8], refs: &[api::ExprTicket]) -> orchid_api::Atom {
		let refs = refs.iter().map(|tk| Expr::new(Arc::new(ExprHandle::from_args(ctx.clone(), *tk))));
		let obj = T::deserialize(DeserCtxImpl(data, &ctx), T::Refs::from_iter(refs));
		obj._factory().build(ctx)
	}
}

pub trait DeserializeCtx: Sized {
	fn read<T: Decode>(&mut self) -> T;
	fn is_empty(&self) -> bool;
	fn assert_empty(self) { assert!(self.is_empty(), "Bytes found after decoding") }
	fn decode<T: Decode>(mut self) -> T {
		let t = self.read();
		self.assert_empty();
		t
	}
	fn sys(&self) -> SysCtx;
}

struct DeserCtxImpl<'a>(&'a [u8], &'a SysCtx);
impl DeserializeCtx for DeserCtxImpl<'_> {
	fn read<T: Decode>(&mut self) -> T { T::decode(&mut self.0) }
	fn is_empty(&self) -> bool { self.0.is_empty() }
	fn sys(&self) -> SysCtx { self.1.clone() }
}

pub trait RefSet {
	fn from_iter<I: Iterator<Item = Expr> + ExactSizeIterator>(refs: I) -> Self;
	fn to_vec(self) -> Vec<Expr>;
}

static E_NON_SER: &str = "Never is a stand-in refset for non-serializable atoms";

impl RefSet for Never {
	fn from_iter<I>(_: I) -> Self { panic!("{E_NON_SER}") }
	fn to_vec(self) -> Vec<Expr> { panic!("{E_NON_SER}") }
}

impl RefSet for () {
	fn to_vec(self) -> Vec<Expr> { Vec::new() }
	fn from_iter<I: Iterator<Item = Expr> + ExactSizeIterator>(refs: I) -> Self {
		assert_eq!(refs.len(), 0, "Expected no refs")
	}
}

impl RefSet for Vec<Expr> {
	fn from_iter<I: Iterator<Item = Expr> + ExactSizeIterator>(refs: I) -> Self { refs.collect_vec() }
	fn to_vec(self) -> Vec<Expr> { self }
}

impl<const N: usize> RefSet for [Expr; N] {
	fn to_vec(self) -> Vec<Expr> { self.into_iter().collect_vec() }
	fn from_iter<I: Iterator<Item = Expr> + ExactSizeIterator>(refs: I) -> Self {
		assert_eq!(refs.len(), N, "Wrong number of refs provided");
		refs.collect_vec().try_into().unwrap_or_else(|_: Vec<_>| unreachable!())
	}
}

/// Atoms that have a [Drop]
pub trait OwnedAtom: Atomic<Variant = OwnedVariant> + Send + Sync + Any + Clone + 'static {
	/// If serializable, the collection that best stores subexpression references
	/// for this atom.
	///
	/// - `()` for no subexppressions,
	/// - `[Expr; N]` for a static number of subexpressions
	/// - `Vec<Expr>` for a variable number of subexpressions
	/// - `Never` if not serializable
	///
	/// If this isn't `Never`, you must override the default, panicking
	/// `serialize` and `deserialize` implementation
	type Refs: RefSet;
	fn val(&self) -> Cow<'_, Self::Data>;
	#[allow(unused_variables)]
	fn call_ref(&self, arg: ExprHandle) -> Expr { bot([err_not_callable()]) }
	fn call(self, arg: ExprHandle) -> Expr {
		let ctx = arg.get_ctx();
		let gcl = self.call_ref(arg);
		self.free(ctx);
		gcl
	}
	#[allow(unused_variables)]
	fn command(self, ctx: SysCtx) -> OrcRes<Option<Expr>> { Err(err_not_command().into()) }
	#[allow(unused_variables)]
	fn free(self, ctx: SysCtx) {}
	#[allow(unused_variables)]
	fn print(&self, ctx: SysCtx) -> String { format!("OwnedAtom({})", type_name::<Self>()) }
	#[allow(unused_variables)]
	fn serialize(&self, ctx: SysCtx, write: &mut (impl Write + ?Sized)) -> Self::Refs {
		assert!(
			TypeId::of::<Self::Refs>() != TypeId::of::<Never>(),
			"The extension scaffold is broken, this function should never be called on Never Refs"
		);
		panic!("Either implement serialize or set Refs to Never for {}", type_name::<Self>())
	}
	#[allow(unused_variables)]
	fn deserialize(ctx: impl DeserializeCtx, refs: Self::Refs) -> Self {
		assert!(
			TypeId::of::<Self::Refs>() != TypeId::of::<Never>(),
			"The extension scaffold is broken, this function should never be called on Never Refs"
		);
		panic!("Either implement deserialize or set Refs to Never for {}", type_name::<Self>())
	}
}
pub trait DynOwnedAtom: Send + Sync + 'static {
	fn atom_tid(&self) -> TypeId;
	fn as_any_ref(&self) -> &dyn Any;
	fn encode(&self, buffer: &mut dyn Write);
	fn dyn_call_ref(&self, ctx: SysCtx, arg: api::ExprTicket) -> Expr;
	fn dyn_call(self: Box<Self>, ctx: SysCtx, arg: api::ExprTicket) -> Expr;
	fn dyn_command(self: Box<Self>, ctx: SysCtx) -> OrcRes<Option<Expr>>;
	fn dyn_free(self: Box<Self>, ctx: SysCtx);
	fn dyn_print(&self, ctx: SysCtx) -> String;
	fn dyn_serialize(&self, ctx: SysCtx, sink: &mut dyn Write) -> Option<Vec<Expr>>;
}
impl<T: OwnedAtom> DynOwnedAtom for T {
	fn atom_tid(&self) -> TypeId { TypeId::of::<T>() }
	fn as_any_ref(&self) -> &dyn Any { self }
	fn encode(&self, buffer: &mut dyn Write) { self.val().as_ref().encode(buffer) }
	fn dyn_call_ref(&self, ctx: SysCtx, arg: api::ExprTicket) -> Expr {
		self.call_ref(ExprHandle::from_args(ctx, arg))
	}
	fn dyn_call(self: Box<Self>, ctx: SysCtx, arg: api::ExprTicket) -> Expr {
		self.call(ExprHandle::from_args(ctx, arg))
	}
	fn dyn_command(self: Box<Self>, ctx: SysCtx) -> OrcRes<Option<Expr>> { self.command(ctx) }
	fn dyn_free(self: Box<Self>, ctx: SysCtx) { self.free(ctx) }
	fn dyn_print(&self, ctx: SysCtx) -> String { self.print(ctx) }
	fn dyn_serialize(&self, ctx: SysCtx, sink: &mut dyn Write) -> Option<Vec<Expr>> {
		(TypeId::of::<Never>() != TypeId::of::<<Self as OwnedAtom>::Refs>())
			.then(|| self.serialize(ctx, sink).to_vec())
	}
}

pub(crate) static OBJ_STORE: IdStore<Box<dyn DynOwnedAtom>> = IdStore::new();
