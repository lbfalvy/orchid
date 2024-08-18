use std::marker::PhantomData;
use std::mem::size_of;

use crate::api;
use crate::system::{DynSystemCard, SystemCard};

pub struct SystemHandle<C: SystemCard> {
  pub(crate) _card: PhantomData<C>,
  pub(crate) id: api::SysId,
}
impl<C: SystemCard> SystemHandle<C> {
  pub(crate) fn new(id: api::SysId) -> Self { Self { _card: PhantomData, id } }
  pub fn id(&self) -> api::SysId { self.id }
}
impl<C: SystemCard> Clone for SystemHandle<C> {
  fn clone(&self) -> Self { Self::new(self.id) }
}

pub trait DynSystemHandle {
  fn id(&self) -> api::SysId;
  fn get_card(&self) -> &dyn DynSystemCard;
}

pub fn leak_card<T: Default>() -> &'static T {
  const {
    if 0 != size_of::<T>() {
      panic!("Attempted to leak positively sized Card. Card types must always be zero-sized");
    }
  }
  Box::leak(Box::default())
}

impl<C: SystemCard> DynSystemHandle for SystemHandle<C> {
  fn id(&self) -> api::SysId { self.id }
  fn get_card(&self) -> &'static dyn DynSystemCard { leak_card::<C>() }
}
