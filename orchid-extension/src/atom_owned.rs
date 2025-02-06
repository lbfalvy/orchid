use std::any::{Any, TypeId, type_name};
use std::borrow::Cow;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use async_once_cell::OnceCell;
use async_std::io::{Read, Write};
use futures::FutureExt;
use futures::future::{LocalBoxFuture, ready};
use itertools::Itertools;
use never::Never;
use orchid_api_traits::{Decode, Encode, enc_vec};
use orchid_base::clone;
use orchid_base::error::OrcRes;
use orchid_base::format::FmtUnit;
use orchid_base::id_store::{IdRecord, IdStore};
use orchid_base::name::Sym;

use crate::api;
use crate::atom::{
	AtomCard, AtomCtx, AtomDynfo, AtomFactory, Atomic, AtomicFeaturesImpl, AtomicVariant, MethodSet,
	MethodSetBuilder, err_not_callable, err_not_command, get_info,
};
use crate::expr::{Expr, ExprHandle};
use crate::gen_expr::{GExpr, bot};
use crate::system::SysCtx;

pub struct OwnedVariant;
impl AtomicVariant for OwnedVariant {}
impl<A: OwnedAtom + Atomic<Variant = OwnedVariant>> AtomicFeaturesImpl<OwnedVariant> for A {
	fn _factory(self) -> AtomFactory {
		AtomFactory::new(move |ctx| async move {
			let rec = ctx.obj_store.add(Box::new(self));
			let (id, _) = get_info::<A>(ctx.cted.inst().card());
			let mut data = enc_vec(&id).await;
			rec.encode(Pin::<&mut Vec<u8>>::new(&mut data)).await;
			api::Atom { drop: Some(api::AtomId(rec.id())), data, owner: ctx.id }
		})
	}
	fn _info() -> Self::_Info { OwnedAtomDynfo { msbuild: A::reg_reqs(), ms: OnceCell::new() } }
	type _Info = OwnedAtomDynfo<A>;
}

fn with_atom<'a, U>(
	id: api::AtomId,
	ctx: &'a SysCtx,
	f: impl FnOnce(IdRecord<'a, Box<dyn DynOwnedAtom>>) -> U,
) -> U {
	f(ctx.obj_store.get(id.0).unwrap_or_else(|| panic!("Received invalid atom ID: {}", id.0)))
}

pub struct OwnedAtomDynfo<T: OwnedAtom> {
	msbuild: MethodSetBuilder<T>,
	ms: OnceCell<MethodSet<T>>,
}
impl<T: OwnedAtom> AtomDynfo for OwnedAtomDynfo<T> {
	fn tid(&self) -> TypeId { TypeId::of::<T>() }
	fn name(&self) -> &'static str { type_name::<T>() }
	fn decode<'a>(&'a self, AtomCtx(data, ..): AtomCtx<'a>) -> LocalBoxFuture<'a, Box<dyn Any>> {
		async {
			Box::new(<T as AtomCard>::Data::decode(Pin::new(&mut &data[..])).await) as Box<dyn Any>
		}
		.boxed_local()
	}
	fn call(&self, AtomCtx(_, id, ctx): AtomCtx, arg: api::ExprTicket) -> LocalBoxFuture<'_, GExpr> {
		with_atom(id.unwrap(), &ctx, |a| a.remove()).dyn_call(ctx.clone(), arg)
	}
	fn call_ref<'a>(
		&'a self,
		AtomCtx(_, id, ctx): AtomCtx<'a>,
		arg: api::ExprTicket,
	) -> LocalBoxFuture<'a, GExpr> {
		async move {
			with_atom(id.unwrap(), &ctx, |a| clone!(ctx; async move { a.dyn_call_ref(ctx, arg).await }))
				.await
		}
		.boxed_local()
	}
	fn print(&self, AtomCtx(_, id, ctx): AtomCtx<'_>) -> LocalBoxFuture<'_, FmtUnit> {
		async move {
			with_atom(id.unwrap(), &ctx, |a| clone!(ctx; async move { a.dyn_print(ctx).await })).await
		}
		.boxed_local()
	}
	fn handle_req<'a, 'b: 'a, 'c: 'a>(
		&'a self,
		AtomCtx(_, id, ctx): AtomCtx,
		key: Sym,
		req: Pin<&'b mut dyn Read>,
		rep: Pin<&'c mut dyn Write>,
	) -> LocalBoxFuture<'a, bool> {
		async move {
			with_atom(id.unwrap(), &ctx, |a| {
				clone!(ctx; async move {
					let ms = self.ms.get_or_init(self.msbuild.pack(ctx.clone())).await;
					ms.dispatch(a.as_any_ref().downcast_ref().unwrap(), ctx, key, req, rep).await
				})
			})
			.await
		}
		.boxed_local()
	}
	fn command<'a>(
		&'a self,
		AtomCtx(_, id, ctx): AtomCtx<'a>,
	) -> LocalBoxFuture<'a, OrcRes<Option<GExpr>>> {
		async move { with_atom(id.unwrap(), &ctx, |a| a.remove().dyn_command(ctx.clone())).await }
			.boxed_local()
	}
	fn drop(&self, AtomCtx(_, id, ctx): AtomCtx) -> LocalBoxFuture<'_, ()> {
		async move { with_atom(id.unwrap(), &ctx, |a| a.remove().dyn_free(ctx.clone())).await }
			.boxed_local()
	}
	fn serialize<'a, 'b: 'a>(
		&'a self,
		AtomCtx(_, id, ctx): AtomCtx<'a>,
		mut write: Pin<&'b mut dyn Write>,
	) -> LocalBoxFuture<'a, Option<Vec<api::ExprTicket>>> {
		async move {
			let id = id.unwrap();
			id.encode(write.as_mut()).await;
			with_atom(id, &ctx, |a| clone!(ctx; async move { a.dyn_serialize(ctx, write).await }))
				.await
				.map(|v| v.into_iter().map(|t| t.handle().tk).collect_vec())
		}
		.boxed_local()
	}
	fn deserialize<'a>(
		&'a self,
		ctx: SysCtx,
		data: &'a [u8],
		refs: &'a [api::ExprTicket],
	) -> LocalBoxFuture<'a, api::Atom> {
		async move {
			let refs =
				refs.iter().map(|tk| Expr::from_handle(Rc::new(ExprHandle::from_args(ctx.clone(), *tk))));
			let obj = T::deserialize(DeserCtxImpl(data, &ctx), T::Refs::from_iter(refs)).await;
			obj._factory().build(ctx).await
		}
		.boxed_local()
	}
}

pub trait DeserializeCtx: Sized {
	fn read<T: Decode>(&mut self) -> impl Future<Output = T>;
	fn is_empty(&self) -> bool;
	fn assert_empty(&self) { assert!(self.is_empty(), "Bytes found after decoding") }
	fn decode<T: Decode>(&mut self) -> impl Future<Output = T> {
		async {
			let t = self.read().await;
			self.assert_empty();
			t
		}
	}
	fn sys(&self) -> SysCtx;
}

struct DeserCtxImpl<'a>(&'a [u8], &'a SysCtx);
impl DeserializeCtx for DeserCtxImpl<'_> {
	async fn read<T: Decode>(&mut self) -> T { T::decode(Pin::new(&mut self.0)).await }
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
pub trait OwnedAtom: Atomic<Variant = OwnedVariant> + Any + Clone + 'static {
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
	fn val(&self) -> impl Future<Output = Cow<'_, Self::Data>>;
	#[allow(unused_variables)]
	fn call_ref(&self, arg: ExprHandle) -> impl Future<Output = GExpr> {
		async move { bot([err_not_callable(&arg.ctx.i).await]) }
	}
	fn call(self, arg: ExprHandle) -> impl Future<Output = GExpr> {
		async {
			let ctx = arg.get_ctx();
			let gcl = self.call_ref(arg).await;
			self.free(ctx).await;
			gcl
		}
	}
	#[allow(unused_variables)]
	fn command(self, ctx: SysCtx) -> impl Future<Output = OrcRes<Option<GExpr>>> {
		async move { Err(err_not_command(&ctx.i).await.into()) }
	}
	#[allow(unused_variables)]
	fn free(self, ctx: SysCtx) -> impl Future<Output = ()> { async {} }
	#[allow(unused_variables)]
	fn print(&self, ctx: SysCtx) -> impl Future<Output = FmtUnit> {
		async { format!("OwnedAtom({})", type_name::<Self>()).into() }
	}
	#[allow(unused_variables)]
	fn serialize(
		&self,
		ctx: SysCtx,
		write: Pin<&mut (impl Write + ?Sized)>,
	) -> impl Future<Output = Self::Refs> {
		assert_serializable::<Self>();
		async { panic!("Either implement serialize or set Refs to Never for {}", type_name::<Self>()) }
	}
	#[allow(unused_variables)]
	fn deserialize(ctx: impl DeserializeCtx, refs: Self::Refs) -> impl Future<Output = Self> {
		assert_serializable::<Self>();
		async {
			panic!("Either implement deserialize or set Refs to Never for {}", type_name::<Self>())
		}
	}
}

fn assert_serializable<T: OwnedAtom>() {
	static MSG: &str = "The extension scaffold is broken, Never Refs should prevent serialization";
	assert_ne!(TypeId::of::<T::Refs>(), TypeId::of::<Never>(), "{MSG}");
}

pub trait DynOwnedAtom: 'static {
	fn atom_tid(&self) -> TypeId;
	fn as_any_ref(&self) -> &dyn Any;
	fn encode<'a>(&'a self, buffer: Pin<&'a mut dyn Write>) -> LocalBoxFuture<'a, ()>;
	fn dyn_call_ref(&self, ctx: SysCtx, arg: api::ExprTicket) -> LocalBoxFuture<'_, GExpr>;
	fn dyn_call(self: Box<Self>, ctx: SysCtx, arg: api::ExprTicket)
	-> LocalBoxFuture<'static, GExpr>;
	fn dyn_command(self: Box<Self>, ctx: SysCtx) -> LocalBoxFuture<'static, OrcRes<Option<GExpr>>>;
	fn dyn_free(self: Box<Self>, ctx: SysCtx) -> LocalBoxFuture<'static, ()>;
	fn dyn_print(&self, ctx: SysCtx) -> LocalBoxFuture<'_, FmtUnit>;
	fn dyn_serialize<'a>(
		&'a self,
		ctx: SysCtx,
		sink: Pin<&'a mut dyn Write>,
	) -> LocalBoxFuture<'a, Option<Vec<Expr>>>;
}
impl<T: OwnedAtom> DynOwnedAtom for T {
	fn atom_tid(&self) -> TypeId { TypeId::of::<T>() }
	fn as_any_ref(&self) -> &dyn Any { self }
	fn encode<'a>(&'a self, buffer: Pin<&'a mut dyn Write>) -> LocalBoxFuture<'a, ()> {
		async { self.val().await.as_ref().encode(buffer).await }.boxed_local()
	}
	fn dyn_call_ref(&self, ctx: SysCtx, arg: api::ExprTicket) -> LocalBoxFuture<'_, GExpr> {
		self.call_ref(ExprHandle::from_args(ctx, arg)).boxed_local()
	}
	fn dyn_call(
		self: Box<Self>,
		ctx: SysCtx,
		arg: api::ExprTicket,
	) -> LocalBoxFuture<'static, GExpr> {
		self.call(ExprHandle::from_args(ctx, arg)).boxed_local()
	}
	fn dyn_command(self: Box<Self>, ctx: SysCtx) -> LocalBoxFuture<'static, OrcRes<Option<GExpr>>> {
		self.command(ctx).boxed_local()
	}
	fn dyn_free(self: Box<Self>, ctx: SysCtx) -> LocalBoxFuture<'static, ()> {
		self.free(ctx).boxed_local()
	}
	fn dyn_print(&self, ctx: SysCtx) -> LocalBoxFuture<'_, FmtUnit> { self.print(ctx).boxed_local() }
	fn dyn_serialize<'a>(
		&'a self,
		ctx: SysCtx,
		sink: Pin<&'a mut dyn Write>,
	) -> LocalBoxFuture<'a, Option<Vec<Expr>>> {
		match TypeId::of::<Never>() == TypeId::of::<<Self as OwnedAtom>::Refs>() {
			true => ready(None).boxed_local(),
			false => async { Some(self.serialize(ctx, sink).await.to_vec()) }.boxed_local(),
		}
	}
}

pub type ObjStore = Rc<IdStore<Box<dyn DynOwnedAtom>>>;
