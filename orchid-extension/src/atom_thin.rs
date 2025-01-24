use std::any::{Any, TypeId, type_name};
use std::future::Future;
use std::pin::Pin;

use async_once_cell::OnceCell;
use async_std::io::{Read, Write};
use futures::FutureExt;
use futures::future::LocalBoxFuture;
use orchid_api_traits::{Coding, enc_vec};
use orchid_base::error::OrcRes;
use orchid_base::name::Sym;

use crate::api;
use crate::atom::{
	AtomCard, AtomCtx, AtomDynfo, AtomFactory, Atomic, AtomicFeaturesImpl, AtomicVariant, MethodSet,
	MethodSetBuilder, err_not_callable, err_not_command, get_info,
};
use crate::expr::ExprHandle;
use crate::gen_expr::{GExpr, bot};
use crate::system::SysCtx;

pub struct ThinVariant;
impl AtomicVariant for ThinVariant {}
impl<A: ThinAtom + Atomic<Variant = ThinVariant>> AtomicFeaturesImpl<ThinVariant> for A {
	fn _factory(self) -> AtomFactory {
		AtomFactory::new(move |ctx| async move {
			let (id, _) = get_info::<A>(ctx.cted.inst().card());
			let mut buf = enc_vec(&id).await;
			self.encode(Pin::new(&mut buf)).await;
			api::Atom { drop: None, data: buf, owner: ctx.id }
		})
	}
	fn _info() -> Self::_Info { ThinAtomDynfo { msbuild: Self::reg_reqs(), ms: OnceCell::new() } }
	type _Info = ThinAtomDynfo<Self>;
}

pub struct ThinAtomDynfo<T: ThinAtom> {
	msbuild: MethodSetBuilder<T>,
	ms: OnceCell<MethodSet<T>>,
}
impl<T: ThinAtom> AtomDynfo for ThinAtomDynfo<T> {
	fn print<'a>(&self, AtomCtx(buf, _, ctx): AtomCtx<'a>) -> LocalBoxFuture<'a, String> {
		async move { T::decode(Pin::new(&mut &buf[..])).await.print(ctx).await }.boxed_local()
	}
	fn tid(&self) -> TypeId { TypeId::of::<T>() }
	fn name(&self) -> &'static str { type_name::<T>() }
	fn decode<'a>(&'a self, AtomCtx(buf, ..): AtomCtx<'a>) -> LocalBoxFuture<'a, Box<dyn Any>> {
		async { Box::new(T::decode(Pin::new(&mut &buf[..])).await) as Box<dyn Any> }.boxed_local()
	}
	fn call<'a>(
		&'a self,
		AtomCtx(buf, _, ctx): AtomCtx<'a>,
		arg: api::ExprTicket,
	) -> LocalBoxFuture<'a, GExpr> {
		Box::pin(async move {
			T::decode(Pin::new(&mut &buf[..])).await.call(ExprHandle::from_args(ctx, arg)).await
		})
	}
	fn call_ref<'a>(
		&'a self,
		AtomCtx(buf, _, ctx): AtomCtx<'a>,
		arg: api::ExprTicket,
	) -> LocalBoxFuture<'a, GExpr> {
		Box::pin(async move {
			T::decode(Pin::new(&mut &buf[..])).await.call(ExprHandle::from_args(ctx, arg)).await
		})
	}
	fn handle_req<'a, 'm1: 'a, 'm2: 'a>(
		&'a self,
		AtomCtx(buf, _, sys): AtomCtx<'a>,
		key: Sym,
		req: Pin<&'m1 mut dyn Read>,
		rep: Pin<&'m2 mut dyn Write>,
	) -> LocalBoxFuture<'a, bool> {
		Box::pin(async move {
			let ms = self.ms.get_or_init(self.msbuild.pack(sys.clone())).await;
			ms.dispatch(&T::decode(Pin::new(&mut &buf[..])).await, sys, key, req, rep).await
		})
	}
	fn command<'a>(
		&'a self,
		AtomCtx(buf, _, ctx): AtomCtx<'a>,
	) -> LocalBoxFuture<'a, OrcRes<Option<GExpr>>> {
		async move { T::decode(Pin::new(&mut &buf[..])).await.command(ctx).await }.boxed_local()
	}
	fn serialize<'a, 'b: 'a>(
		&'a self,
		ctx: AtomCtx<'a>,
		write: Pin<&'b mut dyn Write>,
	) -> LocalBoxFuture<'a, Option<Vec<api::ExprTicket>>> {
		Box::pin(async {
			T::decode(Pin::new(&mut &ctx.0[..])).await.encode(write).await;
			Some(Vec::new())
		})
	}
	fn deserialize<'a>(
		&'a self,
		ctx: SysCtx,
		data: &'a [u8],
		refs: &'a [api::ExprTicket],
	) -> LocalBoxFuture<'a, api::Atom> {
		assert!(refs.is_empty(), "Refs found when deserializing thin atom");
		async { T::decode(Pin::new(&mut &data[..])).await._factory().build(ctx).await }.boxed_local()
	}
	fn drop<'a>(&'a self, AtomCtx(buf, _, ctx): AtomCtx<'a>) -> LocalBoxFuture<'a, ()> {
		async move {
			let string_self = T::decode(Pin::new(&mut &buf[..])).await.print(ctx.clone()).await;
			writeln!(ctx.logger, "Received drop signal for non-drop atom {string_self:?}");
		}
		.boxed_local()
	}
}

pub trait ThinAtom:
	AtomCard<Data = Self> + Atomic<Variant = ThinVariant> + Coding + Send + Sync + 'static
{
	#[allow(unused_variables)]
	fn call(&self, arg: ExprHandle) -> impl Future<Output = GExpr> {
		async move { bot([err_not_callable(&arg.ctx.i).await]) }
	}
	#[allow(unused_variables)]
	fn command(&self, ctx: SysCtx) -> impl Future<Output = OrcRes<Option<GExpr>>> {
		async move { Err(err_not_command(&ctx.i).await.into()) }
	}
	#[allow(unused_variables)]
	fn print(&self, ctx: SysCtx) -> impl Future<Output = String> {
		async { format!("ThinAtom({})", type_name::<Self>()) }
	}
}
