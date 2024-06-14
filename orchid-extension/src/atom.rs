use std::any::{type_name, Any};
use std::io::{Read, Write};
use std::num::NonZeroU64;
use std::ops::Deref;
use std::sync::Arc;
use std::{fmt, mem};

use derive_destructure::destructure;
use orchid_api::atom::{Atom, Fwd};
use orchid_api::expr::{ExprTicket, Release};
use orchid_api::system::SysId;
use orchid_api_traits::{Coding, Decode, Encode, Request};
use orchid_base::id_store::{IdRecord, IdStore};
use orchid_base::reqnot::Requester as _;
use typeid::ConstTypeId;

use crate::expr::GenClause;
use crate::other_system::SystemHandle;
use crate::system::{DynSystemCard, SystemCard};

pub trait AtomCard: 'static + Sized {
  type Owner: SystemCard;
  type Data: Clone + Coding + Sized;
  type Req: Coding;
}

pub fn get_info<A: AtomCard>(sys: &(impl DynSystemCard + ?Sized)) -> (usize, &AtomInfo) {
  sys.atom_info_for(ConstTypeId::of::<A>()).unwrap_or_else(|| {
    panic!("Atom {} not associated with system {}", type_name::<A>(), sys.name())
  })
}

pub fn encode_atom_nodrop<A: AtomCard>(
  sys_id: SysId,
  sys: &(impl DynSystemCard + ?Sized),
  data: &A::Data,
) -> Atom {
  let (info_pos, _) = get_info::<A>(sys);
  let mut buf = (info_pos as u64).enc_vec();
  data.encode(&mut buf);
  Atom { owner: sys_id, drop: false, data: buf }
}

pub fn encode_atom_drop<A: AtomCard>(
  sys_id: SysId,
  sys: &(impl DynSystemCard + ?Sized),
  atom_id: u64,
  data: &A::Data,
) -> Atom {
  let (info_pos, _) = get_info::<A>(sys);
  let mut buf = (info_pos as u64).enc_vec();
  atom_id.encode(&mut buf);
  data.encode(&mut buf);
  Atom { owner: sys_id, drop: true, data: buf }
}

pub fn decode_atom<A: AtomCard>(
  sys: &(impl DynSystemCard + ?Sized),
  Atom { data, drop, owner: _ }: &Atom,
) -> Option<A::Data> {
  let (info_pos, info) = get_info::<A>(sys);
  let mut data = &data[..];
  if u64::decode(&mut data) != info_pos as u64 {
    return None;
  }
  let val = (info.decode)(data);
  Some(*val.downcast().expect("The type-id checked out, the decode should've worked"))
}

#[derive(destructure)]
pub struct ForeignAtom<A: AtomCard> {
  pub(crate) sys: SystemHandle<A::Owner>,
  pub(crate) ticket: ExprTicket,
  pub(crate) api: Atom,
  pub(crate) value: A::Data,
}
impl<A: AtomCard> ForeignAtom<A> {
  /// Unpack the object, returning the held atom and expr ticket. This is in
  /// contrast to dropping the atom which releases the expr ticket.
  pub fn unpack(self) -> (A::Data, ExprTicket, Atom) {
    let (_, ticket, api, value) = self.destructure();
    (value, ticket, api)
  }
  pub fn ticket(&self) -> ExprTicket { self.ticket }
  pub fn request<R: Coding + Into<A::Req> + Request>(&self, req: R) -> R::Response {
    R::Response::decode(&mut &self.sys.reqnot.request(Fwd(self.api.clone(), req.enc_vec()))[..])
  }
}
impl<A: AtomCard> Deref for ForeignAtom<A> {
  type Target = A::Data;
  fn deref(&self) -> &Self::Target { &self.value }
}
impl<A: AtomCard> Drop for ForeignAtom<A> {
  fn drop(&mut self) { self.sys.reqnot.notify(Release(self.sys.id(), self.ticket)) }
}

pub struct AtomInfo {
  pub tid: ConstTypeId,
  pub decode: fn(&[u8]) -> Box<dyn Any>,
  pub call: fn(&[u8], ExprTicket) -> GenClause,
  pub call_ref: fn(&[u8], ExprTicket) -> GenClause,
  pub same: fn(&[u8], &[u8]) -> bool,
  pub handle_req: fn(&[u8], &mut dyn Read, &mut dyn Write),
  pub drop: fn(&[u8]),
}

pub trait ThinAtom: AtomCard<Data = Self> + Coding + fmt::Debug {
  fn call(&self, arg: ExprTicket) -> GenClause;
  fn same(&self, other: &Self) -> bool;
  fn handle_req(&self, req: Self::Req, rep: &mut (impl Write + ?Sized));
}

pub const fn thin_atom_info<T: ThinAtom>() -> AtomInfo {
  AtomInfo {
    tid: ConstTypeId::of::<T>(),
    decode: |mut b| Box::new(T::decode(&mut b)),
    call: |mut b, extk| T::decode(&mut b).call(extk),
    call_ref: |mut b, extk| T::decode(&mut b).call(extk),
    handle_req: |mut b, req, rep| T::decode(&mut b).handle_req(Decode::decode(req), rep),
    same: |mut b1, mut b2| T::decode(&mut b1).same(&T::decode(&mut b2)),
    drop: |mut b1| eprintln!("Received drop signal for non-drop atom {:?}", T::decode(&mut b1)),
  }
}

/// Atoms that have a [Drop]
pub trait OwnedAtom: AtomCard + Deref<Target = Self::Data> + Send + Sync + Any + 'static {
  fn call_ref(&self, arg: ExprTicket) -> GenClause;
  fn call(self, arg: ExprTicket) -> GenClause;
  fn same(&self, other: &Self) -> bool;
  fn handle_req(&self, req: Self::Req, rep: &mut (impl Write + ?Sized));
}

pub trait DynOwnedAtom: Send + Sync + 'static {
  fn atom_tid(&self) -> ConstTypeId;
  fn as_any_ref(&self) -> &dyn Any;
  fn dyn_call_ref(&self, arg: ExprTicket) -> GenClause;
  fn dyn_call(self: Box<Self>, arg: ExprTicket) -> GenClause;
  fn dyn_same(&self, other: &dyn DynOwnedAtom) -> bool;
  fn dyn_handle_req(&self, req: &mut dyn Read, rep: &mut dyn Write);
}

impl<T: OwnedAtom> DynOwnedAtom for T {
  fn atom_tid(&self) -> ConstTypeId { ConstTypeId::of::<T>() }
  fn as_any_ref(&self) -> &dyn Any { self }
  fn dyn_call_ref(&self, arg: ExprTicket) -> GenClause { self.call_ref(arg) }
  fn dyn_call(self: Box<Self>, arg: ExprTicket) -> GenClause { self.call(arg) }
  fn dyn_same(&self, other: &dyn DynOwnedAtom) -> bool {
    if ConstTypeId::of::<Self>() != other.as_any_ref().type_id() {
      return false;
    }
    let other_self = other.as_any_ref().downcast_ref().expect("The type_ids are the same");
    self.same(other_self)
  }
  fn dyn_handle_req(&self, req: &mut dyn Read, rep: &mut dyn Write) {
    self.handle_req(<Self as AtomCard>::Req::decode(req), rep)
  }
}

pub(crate) static OBJ_STORE: IdStore<Box<dyn DynOwnedAtom>> = IdStore::new();

const fn owned_atom_info<T: OwnedAtom>() -> AtomInfo {
  fn with_atom<U>(mut b: &[u8], f: impl FnOnce(IdRecord<'_, Box<dyn DynOwnedAtom>>) -> U) -> U {
    f(OBJ_STORE.get(NonZeroU64::decode(&mut b)).expect("Received invalid atom ID"))
  }
  AtomInfo {
    tid: ConstTypeId::of::<T>(),
    decode: |mut b| Box::new(T::Data::decode(&mut b)),
    call: |b, arg| with_atom(b, |a| a.remove().dyn_call(arg)),
    call_ref: |b, arg| with_atom(b, |a| a.dyn_call_ref(arg)),
    same: |b1, b2| with_atom(b1, |a1| with_atom(b2, |a2| a1.dyn_same(&**a2))),
    handle_req: |b, req, rep| with_atom(b, |a| a.dyn_handle_req(req, rep)),
    drop: |b| mem::drop(with_atom(b, |a| a.remove())),
  }
}
