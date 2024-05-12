use std::marker::PhantomData;
use std::ops::Deref;

use derive_destructure::destructure;
use orchid_api::atom::{Atom, Fwd};
use orchid_api::expr::{ExprTicket, Release};
use orchid_api::proto::ExtMsgSet;
use orchid_api::system::SysId;
use orchid_api_traits::{Coding, Decode, Request};
use orchid_base::reqnot::{ReqNot, Requester};

pub struct SystemHandle<T: SystemDepCard> {
  _t: PhantomData<T>,
  id: SysId,
  reqnot: ReqNot<ExtMsgSet>,
}
impl<T: SystemDepCard> SystemHandle<T> {
  pub(crate) fn new(id: SysId, reqnot: ReqNot<ExtMsgSet>) -> Self {
    Self { _t: PhantomData, id, reqnot }
  }
  pub fn id(&self) -> SysId { self.id }
  pub fn wrap_atom<A: Atomic<Owner = T>>(
    &self,
    api: Atom,
    ticket: ExprTicket,
  ) -> Result<OwnedAtom<A>, Atom> {
    if api.owner == self.id {
      Ok(OwnedAtom { ticket, sys: self.clone(), value: T::decode_atom(&api), api })
    } else {
      Err(api)
    }
  }
}
impl<T: SystemDepCard> Clone for SystemHandle<T> {
  fn clone(&self) -> Self { Self { reqnot: self.reqnot.clone(), _t: PhantomData, id: self.id } }
}

pub trait Atomic: 'static {
  type Owner: SystemDepCard;
  type Req: Coding;
  const HAS_DROP: bool;
}

pub trait SystemDepCard: 'static {
  const NAME: &'static str;
  /// Decode an atom from binary representation.
  ///
  /// This is held in the dep card because there is no global type tag on the
  /// atom, so by the logic of the binary coding algorithm the value has to be a
  /// concrete type, probably an enum of the viable types.
  fn decode_atom<A: Atomic<Owner = Self>>(api: &Atom) -> A;
}

#[derive(destructure)]
pub struct OwnedAtom<A: Atomic> {
  sys: SystemHandle<A::Owner>,
  ticket: ExprTicket,
  api: Atom,
  value: A,
}
impl<A: Atomic> OwnedAtom<A> {
  /// Unpack the object, returning the held atom and expr ticket. This is in
  /// contrast to dropping the atom which releases the expr ticket.
  pub fn unpack(self) -> (A, ExprTicket, Atom) {
    let (_, ticket, api, value) = self.destructure();
    (value, ticket, api)
  }
  pub fn ticket(&self) -> ExprTicket { self.ticket }
  pub fn request<R: Coding + Into<A::Req> + Request>(&self, req: R) -> R::Response {
    R::Response::decode(&mut &self.sys.reqnot.request(Fwd(self.api.clone(), req.enc_vec()))[..])
  }
}
impl<A: Atomic> Deref for OwnedAtom<A> {
  type Target = A;
  fn deref(&self) -> &Self::Target { &self.value }
}
impl<A: Atomic> Drop for OwnedAtom<A> {
  fn drop(&mut self) {
    if A::HAS_DROP {
      self.sys.reqnot.notify(Release(self.sys.id(), self.ticket))
    }
  }
}
