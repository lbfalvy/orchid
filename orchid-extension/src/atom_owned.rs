use std::any::{type_name, Any, TypeId};
use std::borrow::Cow;
use std::io::{Read, Write};
use std::marker::PhantomData;
use std::num::NonZeroU64;

use orchid_api::atom::LocalAtom;
use orchid_api::expr::ExprTicket;
use orchid_api_traits::{Decode, Encode};
use orchid_base::id_store::{IdRecord, IdStore};

use crate::atom::{
  AtomCard, AtomDynfo, AtomFactory, Atomic, AtomicFeaturesImpl, AtomicVariant, ErrorNotCallable,
};
use crate::expr::{bot, ExprHandle, GenExpr};
use crate::system::{atom_info_for, SysCtx};

pub struct OwnedVariant;
impl AtomicVariant for OwnedVariant {}
impl<A: OwnedAtom + Atomic<Variant = OwnedVariant>> AtomicFeaturesImpl<OwnedVariant> for A {
  fn _factory(self) -> AtomFactory {
    AtomFactory::new(move |sys| {
      let rec = OBJ_STORE.add(Box::new(self));
      let mut data = atom_info_for(sys.dyn_card(), rec.atom_tid()).expect("obj exists").0.enc_vec();
      rec.id().encode(&mut data);
      rec.encode(&mut data);
      LocalAtom { drop: true, data }
    })
  }
  type _Info = OwnedAtomDynfo<A>;
  const _INFO: &'static Self::_Info = &OwnedAtomDynfo(PhantomData);
}

fn with_atom<U>(mut b: &[u8], f: impl FnOnce(IdRecord<'_, Box<dyn DynOwnedAtom>>) -> U) -> U {
  f(OBJ_STORE.get(NonZeroU64::decode(&mut b)).expect("Received invalid atom ID"))
}

pub struct OwnedAtomDynfo<T: OwnedAtom>(PhantomData<T>);
impl<T: OwnedAtom> AtomDynfo for OwnedAtomDynfo<T> {
  fn tid(&self) -> TypeId { TypeId::of::<T>() }
  fn decode(&self, data: &[u8]) -> Box<dyn Any> {
    Box::new(<T as AtomCard>::Data::decode(&mut &data[..]))
  }
  fn call(&self, buf: &[u8], ctx: SysCtx, arg: ExprTicket) -> GenExpr {
    with_atom(buf, |a| a.remove().dyn_call(ctx, arg))
  }
  fn call_ref(&self, buf: &[u8], ctx: SysCtx, arg: ExprTicket) -> GenExpr {
    with_atom(buf, |a| a.dyn_call_ref(ctx, arg))
  }
  fn same(&self, buf: &[u8], ctx: SysCtx, buf2: &[u8]) -> bool {
    with_atom(buf, |a1| with_atom(buf2, |a2| a1.dyn_same(ctx, &**a2)))
  }
  fn handle_req(&self, buf: &[u8], ctx: SysCtx, req: &mut dyn Read, rep: &mut dyn Write) {
    with_atom(buf, |a| a.dyn_handle_req(ctx, req, rep))
  }
  fn drop(&self, buf: &[u8], ctx: SysCtx) { with_atom(buf, |a| a.remove().dyn_free(ctx)) }
}

/// Atoms that have a [Drop]
pub trait OwnedAtom: Atomic<Variant = OwnedVariant> + Send + Sync + Any + Clone + 'static {
  fn val(&self) -> Cow<'_, Self::Data>;
  #[allow(unused_variables)]
  fn call_ref(&self, arg: ExprHandle) -> GenExpr { bot(ErrorNotCallable) }
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
  fn handle_req(&self, ctx: SysCtx, req: Self::Req, rep: &mut (impl Write + ?Sized));
  #[allow(unused_variables)]
  fn free(self, ctx: SysCtx) {}
}
pub trait DynOwnedAtom: Send + Sync + 'static {
  fn atom_tid(&self) -> TypeId;
  fn as_any_ref(&self) -> &dyn Any;
  fn encode(&self, buffer: &mut dyn Write);
  fn dyn_call_ref(&self, ctx: SysCtx, arg: ExprTicket) -> GenExpr;
  fn dyn_call(self: Box<Self>, ctx: SysCtx, arg: ExprTicket) -> GenExpr;
  fn dyn_same(&self, ctx: SysCtx, other: &dyn DynOwnedAtom) -> bool;
  fn dyn_handle_req(&self, ctx: SysCtx, req: &mut dyn Read, rep: &mut dyn Write);
  fn dyn_free(self: Box<Self>, ctx: SysCtx);
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
    self.handle_req(ctx, <Self as AtomCard>::Req::decode(req), rep)
  }
  fn dyn_free(self: Box<Self>, ctx: SysCtx) { self.free(ctx) }
}

pub(crate) static OBJ_STORE: IdStore<Box<dyn DynOwnedAtom>> = IdStore::new();
