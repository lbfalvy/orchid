use std::any::{type_name, Any, TypeId};
use std::borrow::Cow;
use std::io::{Read, Write};
use std::marker::PhantomData;

use itertools::Itertools;
use orchid_api_traits::{enc_vec, Decode, Encode};
use orchid_base::error::OrcRes;
use orchid_base::id_store::{IdRecord, IdStore};

use crate::api;
use crate::atom::{
  err_not_callable, err_not_command, get_info, AtomCard, AtomCtx, AtomDynfo, AtomFactory, Atomic,
  AtomicFeaturesImpl, AtomicVariant, ReqPck, RequestPack,
};
use crate::expr::{bot, ExprHandle, GenExpr};
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
  type _Info = OwnedAtomDynfo<A>;
  const _INFO: &'static Self::_Info = &OwnedAtomDynfo(PhantomData);
}

fn with_atom<U>(id: api::AtomId, f: impl FnOnce(IdRecord<'_, Box<dyn DynOwnedAtom>>) -> U) -> U {
  f(OBJ_STORE.get(id.0).unwrap_or_else(|| panic!("Received invalid atom ID: {}", id.0)))
}

pub struct OwnedAtomDynfo<T: OwnedAtom>(PhantomData<T>);
impl<T: OwnedAtom> AtomDynfo for OwnedAtomDynfo<T> {
  fn print(&self, AtomCtx(_, id, ctx): AtomCtx<'_>) -> String {
    with_atom(id.unwrap(), |a| a.dyn_print(ctx))
  }
  fn tid(&self) -> TypeId { TypeId::of::<T>() }
  fn name(&self) -> &'static str { type_name::<T>() }
  fn decode(&self, AtomCtx(data, ..): AtomCtx) -> Box<dyn Any> {
    Box::new(<T as AtomCard>::Data::decode(&mut &data[..]))
  }
  fn call(&self, AtomCtx(_, id, ctx): AtomCtx, arg: api::ExprTicket) -> GenExpr {
    with_atom(id.unwrap(), |a| a.remove().dyn_call(ctx, arg))
  }
  fn call_ref(&self, AtomCtx(_, id, ctx): AtomCtx, arg: api::ExprTicket) -> GenExpr {
    with_atom(id.unwrap(), |a| a.dyn_call_ref(ctx, arg))
  }
  fn same(&self, AtomCtx(_, id, ctx): AtomCtx, a2: &api::Atom) -> bool {
    with_atom(id.unwrap(), |a1| with_atom(a2.drop.unwrap(), |a2| a1.dyn_same(ctx, &**a2)))
  }
  fn handle_req(&self, AtomCtx(_, id, ctx): AtomCtx, req: &mut dyn Read, rep: &mut dyn Write) {
    with_atom(id.unwrap(), |a| a.dyn_handle_req(ctx, req, rep))
  }
  fn command(&self, AtomCtx(_, id, ctx): AtomCtx<'_>) -> OrcRes<Option<GenExpr>> {
    with_atom(id.unwrap(), |a| a.remove().dyn_command(ctx))
  }
  fn drop(&self, AtomCtx(_, id, ctx): AtomCtx) {
    with_atom(id.unwrap(), |a| a.remove().dyn_free(ctx))
  }
  fn serialize(&self, AtomCtx(_, id, ctx): AtomCtx<'_>, write: &mut dyn Write) -> Vec<api::ExprTicket> {
    let id = id.unwrap();
    id.encode(write);
    with_atom(id, |a| a.dyn_serialize(ctx, write)).into_iter().map(|t| t.into_tk()).collect_vec()
  }
  fn deserialize(&self, ctx: SysCtx, data: &[u8], refs: &[api::ExprTicket]) -> orchid_api::Atom {
    let refs = refs.iter().map(|tk| ExprHandle::from_args(ctx.clone(), *tk));
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
impl<'a> DeserializeCtx for DeserCtxImpl<'a> {
  fn read<T: Decode>(&mut self) -> T { T::decode(&mut self.0) }
  fn is_empty(&self) -> bool { self.0.is_empty() }
  fn sys(&self) -> SysCtx { self.1.clone() }
}

pub trait RefSet {
  fn from_iter<I: Iterator<Item = ExprHandle> + ExactSizeIterator>(refs: I) -> Self;
  fn to_vec(self) -> Vec<ExprHandle>;
}

impl RefSet for () {
  fn to_vec(self) -> Vec<ExprHandle> { Vec::new() }
  fn from_iter<I: Iterator<Item = ExprHandle> + ExactSizeIterator>(refs: I) -> Self {
    assert_eq!(refs.len(), 0, "Expected no refs")
  }
}

impl RefSet for Vec<ExprHandle> {
  fn from_iter<I: Iterator<Item = ExprHandle> + ExactSizeIterator>(refs: I) -> Self {
    refs.collect_vec()
  }
  fn to_vec(self) -> Vec<ExprHandle> { self }
}

impl<const N: usize> RefSet for [ExprHandle; N] {
  fn to_vec(self) -> Vec<ExprHandle> { self.into_iter().collect_vec() }
  fn from_iter<I: Iterator<Item = ExprHandle> + ExactSizeIterator>(refs: I) -> Self {
    assert_eq!(refs.len(), N, "Wrong number of refs provided");
    refs.collect_vec().try_into().unwrap_or_else(|_: Vec<_>| unreachable!())
  }
}

/// Atoms that have a [Drop]
pub trait OwnedAtom: Atomic<Variant = OwnedVariant> + Send + Sync + Any + Clone + 'static {
  type Refs: RefSet;
  fn val(&self) -> Cow<'_, Self::Data>;
  #[allow(unused_variables)]
  fn call_ref(&self, arg: ExprHandle) -> GenExpr { bot(err_not_callable()) }
  fn call(self, arg: ExprHandle) -> GenExpr {
    let ctx = arg.get_ctx();
    let gcl = self.call_ref(arg);
    self.free(ctx);
    gcl
  }
  #[allow(unused_variables)]
  fn same(&self, ctx: SysCtx, other: &Self) -> bool {
    let tname = type_name::<Self>();
    writeln!(ctx.logger, "Override OwnedAtom::same for {tname} if it can appear in macro input");
    false
  }
  fn handle_req(&self, pck: impl ReqPck<Self>);
  #[allow(unused_variables)]
  fn command(self, ctx: SysCtx) -> OrcRes<Option<GenExpr>> { Err(vec![err_not_command()]) }
  #[allow(unused_variables)]
  fn free(self, ctx: SysCtx) {}
  #[allow(unused_variables)]
  fn print(&self, ctx: SysCtx) -> String { format!("OwnedAtom({})", type_name::<Self>()) }
  #[allow(unused_variables)]
  fn serialize(&self, ctx: SysCtx, write: &mut (impl Write + ?Sized)) -> Self::Refs;
  fn deserialize(ctx: impl DeserializeCtx, refs: Self::Refs) -> Self;
}
pub trait DynOwnedAtom: Send + Sync + 'static {
  fn atom_tid(&self) -> TypeId;
  fn as_any_ref(&self) -> &dyn Any;
  fn encode(&self, buffer: &mut dyn Write);
  fn dyn_call_ref(&self, ctx: SysCtx, arg: api::ExprTicket) -> GenExpr;
  fn dyn_call(self: Box<Self>, ctx: SysCtx, arg: api::ExprTicket) -> GenExpr;
  fn dyn_same(&self, ctx: SysCtx, other: &dyn DynOwnedAtom) -> bool;
  fn dyn_handle_req(&self, ctx: SysCtx, req: &mut dyn Read, rep: &mut dyn Write);
  fn dyn_command(self: Box<Self>, ctx: SysCtx) -> OrcRes<Option<GenExpr>>;
  fn dyn_free(self: Box<Self>, ctx: SysCtx);
  fn dyn_print(&self, ctx: SysCtx) -> String;
  fn dyn_serialize(&self, ctx: SysCtx, sink: &mut dyn Write) -> Vec<ExprHandle>;
}
impl<T: OwnedAtom> DynOwnedAtom for T {
  fn atom_tid(&self) -> TypeId { TypeId::of::<T>() }
  fn as_any_ref(&self) -> &dyn Any { self }
  fn encode(&self, buffer: &mut dyn Write) { self.val().as_ref().encode(buffer) }
  fn dyn_call_ref(&self, ctx: SysCtx, arg: api::ExprTicket) -> GenExpr {
    self.call_ref(ExprHandle::from_args(ctx, arg))
  }
  fn dyn_call(self: Box<Self>, ctx: SysCtx, arg: api::ExprTicket) -> GenExpr {
    self.call(ExprHandle::from_args(ctx, arg))
  }
  fn dyn_same(&self, ctx: SysCtx, other: &dyn DynOwnedAtom) -> bool {
    if TypeId::of::<Self>() != other.as_any_ref().type_id() {
      return false;
    }
    let other_self = other.as_any_ref().downcast_ref().expect("The type_ids are the same");
    self.same(ctx, other_self)
  }
  fn dyn_handle_req(&self, sys: SysCtx, req: &mut dyn Read, write: &mut dyn Write) {
    let pack = RequestPack::<T, dyn Write>{ req: <Self as AtomCard>::Req::decode(req), write, sys };
    self.handle_req(pack)
  }
  fn dyn_command(self: Box<Self>, ctx: SysCtx) -> OrcRes<Option<GenExpr>> { self.command(ctx) }
  fn dyn_free(self: Box<Self>, ctx: SysCtx) { self.free(ctx) }
  fn dyn_print(&self, ctx: SysCtx) -> String { self.print(ctx) }
  fn dyn_serialize(&self, ctx: SysCtx, sink: &mut dyn Write) -> Vec<ExprHandle> {
    self.serialize(ctx, sink).to_vec()
  }
}

pub(crate) static OBJ_STORE: IdStore<Box<dyn DynOwnedAtom>> = IdStore::new();
