use std::any::{type_name, Any, TypeId};
use std::io::Write;
use std::marker::PhantomData;

use orchid_api::ExprTicket;
use orchid_api_traits::{enc_vec, Coding, Decode};
use orchid_base::error::OrcRes;

use crate::api;
use crate::atom::{
  err_not_callable, err_not_command, get_info, AtomCard, AtomCtx, AtomDynfo, AtomFactory, Atomic,
  AtomicFeaturesImpl, AtomicVariant, ReqPck, RequestPack,
};
use crate::expr::{bot, ExprHandle, GenExpr};
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
  type _Info = ThinAtomDynfo<Self>;
  const _INFO: &'static Self::_Info = &ThinAtomDynfo(PhantomData);
}

pub struct ThinAtomDynfo<T: ThinAtom>(PhantomData<T>);
impl<T: ThinAtom> AtomDynfo for ThinAtomDynfo<T> {
  fn print(&self, AtomCtx(buf, _, ctx): AtomCtx<'_>) -> String {
    T::decode(&mut &buf[..]).print(ctx)
  }
  fn tid(&self) -> TypeId { TypeId::of::<T>() }
  fn name(&self) -> &'static str { type_name::<T>() }
  fn decode(&self, AtomCtx(buf, ..): AtomCtx) -> Box<dyn Any> { Box::new(T::decode(&mut &buf[..])) }
  fn call(&self, AtomCtx(buf, _, ctx): AtomCtx, arg: api::ExprTicket) -> GenExpr {
    T::decode(&mut &buf[..]).call(ExprHandle::from_args(ctx, arg))
  }
  fn call_ref(&self, AtomCtx(buf, _, ctx): AtomCtx, arg: api::ExprTicket) -> GenExpr {
    T::decode(&mut &buf[..]).call(ExprHandle::from_args(ctx, arg))
  }
  fn handle_req(
    &self,
    AtomCtx(buf, _, sys): AtomCtx,
    req: &mut dyn std::io::Read,
    write: &mut dyn Write,
  ) {
    let pack = RequestPack::<T, dyn Write> { req: Decode::decode(req), write, sys };
    T::decode(&mut &buf[..]).handle_req(pack)
  }
  fn same(&self, AtomCtx(buf, _, ctx): AtomCtx, a2: &api::Atom) -> bool {
    T::decode(&mut &buf[..]).same(ctx, &T::decode(&mut &a2.data[8..]))
  }
  fn command(&self, AtomCtx(buf, _, ctx): AtomCtx<'_>) -> OrcRes<Option<GenExpr>> {
    T::decode(&mut &buf[..]).command(ctx)
  }
  fn serialize(&self, AtomCtx(buf, ..): AtomCtx<'_>, write: &mut dyn Write) -> Vec<ExprTicket> {
    T::decode(&mut &buf[..]).encode(write);
    Vec::new()
  }
  fn deserialize(&self, ctx: SysCtx, data: &[u8], refs: &[ExprTicket]) -> api::Atom {
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
  fn call(&self, arg: ExprHandle) -> GenExpr { bot(err_not_callable()) }
  #[allow(unused_variables)]
  fn same(&self, ctx: SysCtx, other: &Self) -> bool {
    let tname = type_name::<Self>();
    writeln!(ctx.logger, "Override ThinAtom::same for {tname} if it can appear in macro input");
    false
  }
  fn handle_req(&self, pck: impl ReqPck<Self>);
  #[allow(unused_variables)]
  fn command(&self, ctx: SysCtx) -> OrcRes<Option<GenExpr>> { Err(vec![err_not_command()]) }
  #[allow(unused_variables)]
  fn print(&self, ctx: SysCtx) -> String { format!("ThinAtom({})", type_name::<Self>()) }
}
