use std::any::{Any, TypeId, type_name};
use std::future::Future;
use std::io::Write;

use futures::FutureExt;
use futures::future::LocalBoxFuture;
use orchid_api_traits::{Coding, enc_vec};
use orchid_base::error::OrcRes;
use orchid_base::name::Sym;

use crate::api;
use crate::atom::{
	AtomCard, AtomCtx, AtomDynfo, AtomFactory, Atomic, AtomicFeaturesImpl, AtomicVariant, MethodSet,
	err_not_callable, err_not_command, get_info,
};
use crate::expr::{Expr, ExprHandle, bot};
use crate::system::SysCtx;

pub struct ThinVariant;
impl AtomicVariant for ThinVariant {}
impl<A: ThinAtom + Atomic<Variant = ThinVariant>> AtomicFeaturesImpl<ThinVariant> for A {
	fn _factory(self) -> AtomFactory {
		AtomFactory::new(move |ctx| {
			let (id, _) = get_info::<A>(ctx.cted.inst().card());
			let mut buf = enc_vec(&id);
			self.encode(&mut buf);
			api::Atom { drop: None, data: buf, owner: ctx.id }
		})
	}
	fn _info() -> Self::_Info { ThinAtomDynfo(Self::reg_reqs()) }
	type _Info = ThinAtomDynfo<Self>;
}

pub struct ThinAtomDynfo<T: ThinAtom>(MethodSet<T>);
impl<T: ThinAtom> AtomDynfo for ThinAtomDynfo<T> {
	fn print<'a>(&self, AtomCtx(buf, _, ctx): AtomCtx<'a>) -> LocalBoxFuture<'a, String> {
		async move { T::decode(&mut &buf[..]).print(ctx).await }.boxed_local()
	}
	fn tid(&self) -> TypeId { TypeId::of::<T>() }
	fn name(&self) -> &'static str { type_name::<T>() }
	fn decode(&self, AtomCtx(buf, ..): AtomCtx) -> Box<dyn Any> { Box::new(T::decode(&mut &buf[..])) }
	fn call<'a>(
		&'a self,
		AtomCtx(buf, _, ctx): AtomCtx<'a>,
		arg: api::ExprTicket,
	) -> LocalBoxFuture<'a, Expr> {
		async move { T::decode(&mut &buf[..]).call(ExprHandle::from_args(ctx, arg)).await }
			.boxed_local()
	}
	fn call_ref<'a>(
		&'a self,
		AtomCtx(buf, _, ctx): AtomCtx<'a>,
		arg: api::ExprTicket,
	) -> LocalBoxFuture<'a, Expr> {
		async move { T::decode(&mut &buf[..]).call(ExprHandle::from_args(ctx, arg)).await }
			.boxed_local()
	}
	fn handle_req<'a, 'm1: 'a, 'm2: 'a>(
		&'a self,
		AtomCtx(buf, _, sys): AtomCtx<'a>,
		key: Sym,
		req: &'m1 mut dyn std::io::Read,
		rep: &'m2 mut dyn Write,
	) -> LocalBoxFuture<'a, bool> {
		async move { self.0.dispatch(&T::decode(&mut &buf[..]), sys, key, req, rep).await }
			.boxed_local()
	}
	fn command<'a>(
		&'a self,
		AtomCtx(buf, _, ctx): AtomCtx<'a>,
	) -> LocalBoxFuture<'a, OrcRes<Option<Expr>>> {
		async move { T::decode(&mut &buf[..]).command(ctx).await }.boxed_local()
	}
	fn serialize<'a, 'b: 'a>(
		&'a self,
		ctx: AtomCtx<'a>,
		write: &'b mut dyn Write,
	) -> LocalBoxFuture<'a, Option<Vec<api::ExprTicket>>> {
		T::decode(&mut &ctx.0[..]).encode(write);
		async { Some(Vec::new()) }.boxed_local()
	}
	fn deserialize<'a>(
		&'a self,
		ctx: SysCtx,
		data: &'a [u8],
		refs: &'a [api::ExprTicket],
	) -> LocalBoxFuture<'a, api::Atom> {
		assert!(refs.is_empty(), "Refs found when deserializing thin atom");
		async { T::decode(&mut &data[..])._factory().build(ctx) }.boxed_local()
	}
	fn drop<'a>(&'a self, AtomCtx(buf, _, ctx): AtomCtx<'a>) -> LocalBoxFuture<'a, ()> {
		async move {
			let string_self = T::decode(&mut &buf[..]).print(ctx.clone()).await;
			writeln!(ctx.logger, "Received drop signal for non-drop atom {string_self:?}");
		}
		.boxed_local()
	}
}

pub trait ThinAtom:
	AtomCard<Data = Self> + Atomic<Variant = ThinVariant> + Coding + Send + Sync + 'static
{
	#[allow(unused_variables)]
	fn call(&self, arg: ExprHandle) -> impl Future<Output = Expr> {
		async { bot([err_not_callable().await]) }
	}
	#[allow(unused_variables)]
	fn command(&self, ctx: SysCtx) -> impl Future<Output = OrcRes<Option<Expr>>> {
		async { Err(err_not_command().await.into()) }
	}
	#[allow(unused_variables)]
	fn print(&self, ctx: SysCtx) -> impl Future<Output = String> {
		async { format!("ThinAtom({})", type_name::<Self>()) }
	}
}
