use std::any::{type_name, Any};
use std::io::{Read, Write};
use std::ops::Deref;

use dyn_clone::{clone_box, DynClone};
use orchid_api::atom::{Atom, Fwd, LocalAtom};
use orchid_api::expr::ExprTicket;
use orchid_api_traits::{Coding, Decode, Request};
use orchid_base::location::Pos;
use orchid_base::reqnot::Requester;
use trait_set::trait_set;
use typeid::ConstTypeId;

use crate::error::ProjectError;
use crate::expr::{ExprHandle, GenExpr};
use crate::system::{atom_info_for, DynSystem, DynSystemCard, SysCtx};

pub trait AtomCard: 'static + Sized {
  type Data: Clone + Coding + Sized;
  type Req: Coding;
}

pub trait AtomicVariant {}
pub trait Atomic: 'static + Sized {
  type Variant: AtomicVariant;
  type Data: Clone + Coding + Sized;
  type Req: Coding;
}
impl<A: Atomic> AtomCard for A {
  type Data = <Self as Atomic>::Data;
  type Req = <Self as Atomic>::Req;
}

pub trait AtomicFeatures: Atomic {
  fn factory(self) -> AtomFactory;
  fn info() -> &'static AtomInfo;
}
pub trait AtomicFeaturesImpl<Variant: AtomicVariant> {
  fn _factory(self) -> AtomFactory;
  fn _info() -> &'static AtomInfo;
}
impl<A: Atomic + AtomicFeaturesImpl<A::Variant>> AtomicFeatures for A {
  fn factory(self) -> AtomFactory { self._factory() }
  fn info() -> &'static AtomInfo { Self::_info() }
}

pub fn get_info<A: AtomCard>(sys: &(impl DynSystemCard + ?Sized)) -> (u64, &AtomInfo) {
  atom_info_for(sys, ConstTypeId::of::<A>()).unwrap_or_else(|| {
    panic!("Atom {} not associated with system {}", type_name::<A>(), sys.name())
  })
}

#[derive(Clone)]
pub struct ForeignAtom {
  pub expr: ExprHandle,
  pub atom: Atom,
  pub pos: Pos,
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
  pub call: fn(&[u8], SysCtx, ExprTicket) -> GenExpr,
  pub call_ref: fn(&[u8], SysCtx, ExprTicket) -> GenExpr,
  pub same: fn(&[u8], SysCtx, &[u8]) -> bool,
  pub handle_req: fn(&[u8], SysCtx, &mut dyn Read, &mut dyn Write),
  pub drop: fn(&[u8], SysCtx),
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

pub struct ErrorNotCallable;
impl ProjectError for ErrorNotCallable {
  const DESCRIPTION: &'static str = "This atom is not callable";
}