use std::any::{type_name, Any};
use std::borrow::Cow;
use std::fmt;
use std::io::{Read, Write};
use std::num::NonZeroU64;
use std::ops::Deref;

use dyn_clone::{clone_box, DynClone};
use orchid_api::atom::{Atom, Fwd, LocalAtom};
use orchid_api::expr::ExprTicket;
use orchid_api::system::SysId;
use orchid_api_traits::{Coding, Decode, Encode, Request};
use orchid_base::id_store::{IdRecord, IdStore};
use orchid_base::location::Pos;
use orchid_base::reqnot::Requester;
use trait_set::trait_set;
use typeid::ConstTypeId;

use crate::expr::{bot, ExprHandle, GenClause};
use crate::system::{atom_info_for, DynSystem, DynSystemCard, SysCtx};

pub trait AtomCard: 'static + Sized {
  // type Owner: SystemCard;
  type Data: Clone + Coding + Sized;
  type Req: Coding;
}

pub fn get_info<A: AtomCard>(sys: &(impl DynSystemCard + ?Sized)) -> (u64, &AtomInfo) {
  atom_info_for(sys, ConstTypeId::of::<A>()).unwrap_or_else(|| {
    panic!("Atom {} not associated with system {}", type_name::<A>(), sys.name())
  })
}

pub fn encode_atom_nodrop<A: AtomCard>(
  sys: &(impl DynSystemCard + ?Sized),
  data: &A::Data,
) -> LocalAtom {
  let mut buf = get_info::<A>(sys).0.enc_vec();
  data.encode(&mut buf);
  LocalAtom { drop: false, data: buf }
}

pub fn encode_atom_drop<A: AtomCard>(
  sys_id: SysId,
  sys: &(impl DynSystemCard + ?Sized),
  atom_id: u64,
  data: &A::Data,
) -> Atom {
  let mut buf = get_info::<A>(sys).0.enc_vec();
  atom_id.encode(&mut buf);
  data.encode(&mut buf);
  Atom { owner: sys_id, drop: true, data: buf }
}

pub fn decode_atom<A: AtomCard>(
  sys: &(impl DynSystemCard + ?Sized),
  Atom { data, drop: _, owner: _ }: &Atom,
) -> Option<A::Data> {
  let (info_pos, info) = get_info::<A>(sys);
  let mut data = &data[..];
  if u64::decode(&mut data) != info_pos {
    return None;
  }
  let val = (info.decode)(data);
  Some(*val.downcast().expect("The type-id checked out, the decode should've worked"))
}

#[derive(Clone)]
pub struct ForeignAtom {
  pub expr: ExprHandle,
  pub atom: Atom,
  pub position: Pos,
}
impl ForeignAtom {}

#[derive(Clone)]
pub struct TypAtom<A: AtomCard> {
  pub data: ForeignAtom,
  pub value: A::Data,
}
impl<A: AtomCard> TypAtom<A> {
  pub fn request<R: Coding + Into<A::Req> + Request>(&self, req: R) -> R::Response {
    R::Response::decode(
      &mut &self.data.expr.ctx.reqnot.request(Fwd(self.data.atom.clone(), req.enc_vec()))[..],
    )
  }
}
impl<A: AtomCard> Deref for TypAtom<A> {
  type Target = A::Data;
  fn deref(&self) -> &Self::Target { &self.value }
}

pub struct AtomInfo {
  pub tid: ConstTypeId,
  pub decode: fn(&[u8]) -> Box<dyn Any>,
  pub call: fn(&[u8], SysCtx, ExprTicket) -> GenClause,
  pub call_ref: fn(&[u8], SysCtx, ExprTicket) -> GenClause,
  pub same: fn(&[u8], SysCtx, &[u8]) -> bool,
  pub handle_req: fn(&[u8], SysCtx, &mut dyn Read, &mut dyn Write),
  pub drop: fn(&[u8], SysCtx),
}

pub trait ThinAtom: AtomCard<Data = Self> + Coding + fmt::Debug {
  #[allow(unused_variables)]
  fn call(&self, arg: ExprHandle) -> GenClause { bot("This atom is not callable") }
  #[allow(unused_variables)]
  fn same(&self, ctx: SysCtx, other: &Self) -> bool {
    eprintln!(
      "Override ThinAtom::same for {} if it can be generated during parsing",
      type_name::<Self>()
    );
    false
  }
  fn handle_req(&self, ctx: SysCtx, req: Self::Req, rep: &mut (impl Write + ?Sized));
  fn factory(self) -> AtomFactory {
    AtomFactory::new(move |sys| encode_atom_nodrop::<Self>(sys.dyn_card(), &self))
  }
}

pub const fn thin_atom_info<T: ThinAtom>() -> AtomInfo {
  AtomInfo {
    tid: ConstTypeId::of::<T>(),
    decode: |mut b| Box::new(T::decode(&mut b)),
    call: |mut b, ctx, extk| T::decode(&mut b).call(ExprHandle::from_args(ctx, extk)),
    call_ref: |mut b, ctx, extk| T::decode(&mut b).call(ExprHandle::from_args(ctx, extk)),
    handle_req: |mut b, ctx, req, rep| T::decode(&mut b).handle_req(ctx, Decode::decode(req), rep),
    same: |mut b1, ctx, mut b2| T::decode(&mut b1).same(ctx, &T::decode(&mut b2)),
    drop: |mut b1, _| eprintln!("Received drop signal for non-drop atom {:?}", T::decode(&mut b1)),
  }
}

/// Atoms that have a [Drop]
pub trait OwnedAtom: AtomCard + Send + Sync + Any + Clone + 'static {
  fn val(&self) -> Cow<'_, Self::Data>;
  #[allow(unused_variables)]
  fn call_ref(&self, arg: ExprHandle) -> GenClause { bot("This atom is not callable") }
  fn call(self, arg: ExprHandle) -> GenClause {
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
  #[allow(unused_variables)]
  fn factory(self) -> AtomFactory {
    AtomFactory::new(move |sys| {
      let rec = OBJ_STORE.add(Box::new(self));
      let mut data = atom_info_for(sys.dyn_card(), rec.atom_tid()).expect("obj exists").0.enc_vec();
      rec.id().encode(&mut data);
      rec.encode(&mut data);
      LocalAtom { drop: true, data }
    })
  }
}

pub trait DynOwnedAtom: Send + Sync + 'static {
  fn atom_tid(&self) -> ConstTypeId;
  fn as_any_ref(&self) -> &dyn Any;
  fn encode(&self, buffer: &mut dyn Write);
  fn dyn_call_ref(&self, ctx: SysCtx, arg: ExprTicket) -> GenClause;
  fn dyn_call(self: Box<Self>, ctx: SysCtx, arg: ExprTicket) -> GenClause;
  fn dyn_same(&self, ctx: SysCtx, other: &dyn DynOwnedAtom) -> bool;
  fn dyn_handle_req(&self, ctx: SysCtx, req: &mut dyn Read, rep: &mut dyn Write);
  fn dyn_free(self: Box<Self>, ctx: SysCtx);
}
impl<T: OwnedAtom> DynOwnedAtom for T {
  fn atom_tid(&self) -> ConstTypeId { ConstTypeId::of::<T>() }
  fn as_any_ref(&self) -> &dyn Any { self }
  fn encode(&self, buffer: &mut dyn Write) { self.val().as_ref().encode(buffer) }
  fn dyn_call_ref(&self, ctx: SysCtx, arg: ExprTicket) -> GenClause {
    self.call_ref(ExprHandle::from_args(ctx, arg))
  }
  fn dyn_call(self: Box<Self>, ctx: SysCtx, arg: ExprTicket) -> GenClause {
    self.call(ExprHandle::from_args(ctx, arg))
  }
  fn dyn_same(&self, ctx: SysCtx, other: &dyn DynOwnedAtom) -> bool {
    if ConstTypeId::of::<Self>() != other.as_any_ref().type_id() {
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

pub const fn owned_atom_info<T: OwnedAtom>() -> AtomInfo {
  fn with_atom<U>(mut b: &[u8], f: impl FnOnce(IdRecord<'_, Box<dyn DynOwnedAtom>>) -> U) -> U {
    f(OBJ_STORE.get(NonZeroU64::decode(&mut b)).expect("Received invalid atom ID"))
  }
  AtomInfo {
    tid: ConstTypeId::of::<T>(),
    decode: |mut b| Box::new(T::Data::decode(&mut b)),
    call: |b, ctx, arg| with_atom(b, |a| a.remove().dyn_call(ctx, arg)),
    call_ref: |b, ctx, arg| with_atom(b, |a| a.dyn_call_ref(ctx, arg)),
    same: |b1, ctx, b2| with_atom(b1, |a1| with_atom(b2, |a2| a1.dyn_same(ctx, &**a2))),
    handle_req: |b, ctx, req, rep| with_atom(b, |a| a.dyn_handle_req(ctx, req, rep)),
    drop: |b, ctx| with_atom(b, |a| a.remove().dyn_free(ctx)),
  }
}

trait_set! {
  pub trait AtomFactoryFn = FnOnce(&dyn DynSystem) -> LocalAtom + DynClone;
}
pub struct AtomFactory(Box<dyn AtomFactoryFn>);
impl AtomFactory {
  pub fn new(f: impl FnOnce(&dyn DynSystem) -> LocalAtom + Clone + 'static) -> Self {
    Self(Box::new(f))
  }
  pub fn build(self, sys: &dyn DynSystem) -> LocalAtom { (self.0)(sys) }
}
impl Clone for AtomFactory {
  fn clone(&self) -> Self { AtomFactory(clone_box(&*self.0)) }
}
