use std::any::{Any, TypeId, type_name};
use std::io::Write;

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
	fn print(&self, AtomCtx(buf, _, ctx): AtomCtx<'_>) -> String {
		T::decode(&mut &buf[..]).print(ctx)
	}
	fn tid(&self) -> TypeId { TypeId::of::<T>() }
	fn name(&self) -> &'static str { type_name::<T>() }
	fn decode(&self, AtomCtx(buf, ..): AtomCtx) -> Box<dyn Any> { Box::new(T::decode(&mut &buf[..])) }
	fn call(&self, AtomCtx(buf, _, ctx): AtomCtx, arg: api::ExprTicket) -> Expr {
		T::decode(&mut &buf[..]).call(ExprHandle::from_args(ctx, arg))
	}
	fn call_ref(&self, AtomCtx(buf, _, ctx): AtomCtx, arg: api::ExprTicket) -> Expr {
		T::decode(&mut &buf[..]).call(ExprHandle::from_args(ctx, arg))
	}
	fn handle_req(
		&self,
		AtomCtx(buf, _, sys): AtomCtx,
		key: Sym,
		req: &mut dyn std::io::Read,
		rep: &mut dyn Write,
	) -> bool {
		self.0.dispatch(&T::decode(&mut &buf[..]), sys, key, req, rep)
	}
	fn command(&self, AtomCtx(buf, _, ctx): AtomCtx<'_>) -> OrcRes<Option<Expr>> {
		T::decode(&mut &buf[..]).command(ctx)
	}
	fn serialize(&self, actx: AtomCtx<'_>, write: &mut dyn Write) -> Option<Vec<api::ExprTicket>> {
		T::decode(&mut &actx.0[..]).encode(write);
		Some(Vec::new())
	}
	fn deserialize(&self, ctx: SysCtx, data: &[u8], refs: &[api::ExprTicket]) -> api::Atom {
		assert!(refs.is_empty(), "Refs found when deserializing thin atom");
		T::decode(&mut &data[..])._factory().build(ctx)
	}
	fn drop(&self, AtomCtx(buf, _, ctx): AtomCtx) {
		let string_self = T::decode(&mut &buf[..]).print(ctx.clone());
		writeln!(ctx.logger, "Received drop signal for non-drop atom {string_self:?}");
	}
}

pub trait ThinAtom:
	AtomCard<Data = Self> + Atomic<Variant = ThinVariant> + Coding + Send + Sync + 'static
{
	#[allow(unused_variables)]
	fn call(&self, arg: ExprHandle) -> Expr { bot([err_not_callable()]) }
	#[allow(unused_variables)]
	fn command(&self, ctx: SysCtx) -> OrcRes<Option<Expr>> { Err(err_not_command().into()) }
	#[allow(unused_variables)]
	fn print(&self, ctx: SysCtx) -> String { format!("ThinAtom({})", type_name::<Self>()) }
}
