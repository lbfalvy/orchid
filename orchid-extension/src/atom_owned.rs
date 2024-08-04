use std::any::{type_name, Any, TypeId};
use std::borrow::Cow;
use std::io::{Read, Write};
use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::sync::Arc;

use orchid_api::atom::LocalAtom;
use orchid_api::expr::ExprTicket;
use orchid_api_traits::{Decode, Encode};
use orchid_base::id_store::{IdRecord, IdStore};

use crate::atom::{
  get_info, AtomCard, AtomCtx, AtomDynfo, AtomFactory, Atomic, AtomicFeaturesImpl, AtomicVariant, ErrNotCallable, ErrorNotCommand, ReqPck, RequestPack
};
use crate::error::ProjectResult;
use crate::expr::{bot, ExprHandle, GenExpr};
use crate::system::SysCtx;

pub struct OwnedVariant;
impl AtomicVariant for OwnedVariant {}
impl<A: OwnedAtom + Atomic<Variant = OwnedVariant>> AtomicFeaturesImpl<OwnedVariant> for A {
  fn _factory(self) -> AtomFactory {
    AtomFactory::new(move |sys| {
      let rec = OBJ_STORE.add(Box::new(self));
      let mut data = get_info::<A>(sys.dyn_card()).0.enc_vec();
      rec.id().encode(&mut data);
      rec.encode(&mut data);
      LocalAtom { drop: true, data }
    })
  }
  type _Info = OwnedAtomDynfo<A>;
  const _INFO: &'static Self::_Info = &OwnedAtomDynfo(PhantomData);
}

fn with_atom<U>(mut b: &[u8], f: impl FnOnce(IdRecord<'_, Box<dyn DynOwnedAtom>>) -> U) -> U {
  let id = NonZeroU64::decode(&mut b);
  f(OBJ_STORE.get(id).unwrap_or_else(|| panic!("Received invalid atom ID: {id}")))
}

pub struct OwnedAtomDynfo<T: OwnedAtom>(PhantomData<T>);
impl<T: OwnedAtom> AtomDynfo for OwnedAtomDynfo<T> {
  fn print(&self, AtomCtx(buf, ctx): AtomCtx<'_>) -> String {
    with_atom(buf, |a| a.dyn_print(ctx))
  }
  fn tid(&self) -> TypeId { TypeId::of::<T>() }
  fn name(&self) -> &'static str { type_name::<T>() }
  fn decode(&self, AtomCtx(data, _): AtomCtx) -> Box<dyn Any> {
    Box::new(<T as AtomCard>::Data::decode(&mut &data[..]))
  }
  fn call(&self, AtomCtx(buf, ctx): AtomCtx, arg: ExprTicket) -> GenExpr {
    with_atom(buf, |a| a.remove().dyn_call(ctx, arg))
  }
  fn call_ref(&self, AtomCtx(buf, ctx): AtomCtx, arg: ExprTicket) -> GenExpr {
    with_atom(buf, |a| a.dyn_call_ref(ctx, arg))
  }
  fn same(&self, AtomCtx(buf, ctx): AtomCtx, buf2: &[u8]) -> bool {
    with_atom(buf, |a1| with_atom(buf2, |a2| a1.dyn_same(ctx, &**a2)))
  }
  fn handle_req(&self, AtomCtx(buf, ctx): AtomCtx, req: &mut dyn Read, rep: &mut dyn Write) {
    with_atom(buf, |a| a.dyn_handle_req(ctx, req, rep))
  }
  fn command(&self, AtomCtx(buf, ctx): AtomCtx<'_>) -> ProjectResult<Option<GenExpr>> {
    with_atom(buf, |a| a.remove().dyn_command(ctx))
  }
  fn drop(&self, AtomCtx(buf, ctx): AtomCtx) { with_atom(buf, |a| a.remove().dyn_free(ctx)) }
}

/// Atoms that have a [Drop]
pub trait OwnedAtom: Atomic<Variant = OwnedVariant> + Send + Sync + Any + Clone + 'static {
  fn val(&self) -> Cow<'_, Self::Data>;
  #[allow(unused_variables)]
  fn call_ref(&self, arg: ExprHandle) -> GenExpr { bot(ErrNotCallable) }
  fn call(self, arg: ExprHandle) -> GenExpr {
    let ctx = arg.get_ctx();
    let gcl = self.call_ref(arg);
    self.free(ctx);
    gcl
  }
  #[allow(unused_variables)]
  fn same(&self, ctx: SysCtx, other: &Self) -> bool {
    eprintln!(
      "Override OwnedAtom::same for {} if it can be generated during parsing",
      type_name::<Self>()
    );
    false
  }
  fn handle_req(&self, ctx: SysCtx, pck: impl ReqPck<Self>);
  #[allow(unused_variables)]
  fn command(self, ctx: SysCtx) -> ProjectResult<Option<GenExpr>> { Err(Arc::new(ErrorNotCommand)) }
  #[allow(unused_variables)]
  fn free(self, ctx: SysCtx) {}
  #[allow(unused_variables)]
  fn print(&self, ctx: SysCtx) -> String { format!("OwnedAtom({})", type_name::<Self>()) }
}
pub trait DynOwnedAtom: Send + Sync + 'static {
  fn atom_tid(&self) -> TypeId;
  fn as_any_ref(&self) -> &dyn Any;
  fn encode(&self, buffer: &mut dyn Write);
  fn dyn_call_ref(&self, ctx: SysCtx, arg: ExprTicket) -> GenExpr;
  fn dyn_call(self: Box<Self>, ctx: SysCtx, arg: ExprTicket) -> GenExpr;
  fn dyn_same(&self, ctx: SysCtx, other: &dyn DynOwnedAtom) -> bool;
  fn dyn_handle_req(&self, ctx: SysCtx, req: &mut dyn Read, rep: &mut dyn Write);
  fn dyn_command(self: Box<Self>, ctx: SysCtx) -> ProjectResult<Option<GenExpr>>;
  fn dyn_free(self: Box<Self>, ctx: SysCtx);
  fn dyn_print(&self, ctx: SysCtx) -> String;
}
impl<T: OwnedAtom> DynOwnedAtom for T {
  fn atom_tid(&self) -> TypeId { TypeId::of::<T>() }
  fn as_any_ref(&self) -> &dyn Any { self }
  fn encode(&self, buffer: &mut dyn Write) { self.val().as_ref().encode(buffer) }
  fn dyn_call_ref(&self, ctx: SysCtx, arg: ExprTicket) -> GenExpr {
    self.call_ref(ExprHandle::from_args(ctx, arg))
  }
  fn dyn_call(self: Box<Self>, ctx: SysCtx, arg: ExprTicket) -> GenExpr {
    self.call(ExprHandle::from_args(ctx, arg))
  }
  fn dyn_same(&self, ctx: SysCtx, other: &dyn DynOwnedAtom) -> bool {
    if TypeId::of::<Self>() != other.as_any_ref().type_id() {
      return false;
    }
    let other_self = other.as_any_ref().downcast_ref().expect("The type_ids are the same");
    self.same(ctx, other_self)
  }
  fn dyn_handle_req(&self, ctx: SysCtx, req: &mut dyn Read, rep: &mut dyn Write) {
    self.handle_req(ctx, RequestPack::<T, dyn Write>(<Self as AtomCard>::Req::decode(req), rep))
  }
  fn dyn_command(self: Box<Self>, ctx: SysCtx) -> ProjectResult<Option<GenExpr>> {
    self.command(ctx)
  }
  fn dyn_free(self: Box<Self>, ctx: SysCtx) { self.free(ctx) }
  fn dyn_print(&self, ctx: SysCtx) -> String { self.print(ctx) }
}

pub(crate) static OBJ_STORE: IdStore<Box<dyn DynOwnedAtom>> = IdStore::new();
