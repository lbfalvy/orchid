use std::marker::PhantomData;

use orchid_api::atom::Atom;
use orchid_api::expr::ExprTicket;
use orchid_api::proto::ExtMsgSet;
use orchid_api::system::SysId;
use orchid_base::reqnot::ReqNot;

use crate::atom::{decode_atom, AtomCard, ForeignAtom};
use crate::system::SystemCard;

pub struct SystemHandle<C: SystemCard> {
  pub(crate) _card: PhantomData<C>,
  pub(crate) id: SysId,
  pub(crate) reqnot: ReqNot<ExtMsgSet>,
}
impl<T: SystemCard> SystemHandle<T> {
  pub(crate) fn new(id: SysId, reqnot: ReqNot<ExtMsgSet>) -> Self {
    Self { _card: PhantomData, id, reqnot }
  }
  pub fn id(&self) -> SysId { self.id }
  pub fn wrap_atom<A: AtomCard<Owner = T>>(
    &self,
    api: Atom,
    ticket: ExprTicket,
  ) -> Result<ForeignAtom<A>, Atom> {
    if api.owner == self.id {
      if let Some(value) = decode_atom::<A>(&T::default(), &api) {
        return Ok(ForeignAtom { ticket, sys: self.clone(), value, api });
      }
    }
    Err(api)
  }
}
impl<T: SystemCard> Clone for SystemHandle<T> {
  fn clone(&self) -> Self { Self { reqnot: self.reqnot.clone(), _card: PhantomData, id: self.id } }
}
