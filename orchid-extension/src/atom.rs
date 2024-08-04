use std::any::{type_name, Any, TypeId};
use std::io::{Read, Write};
use std::ops::Deref;
use std::sync::OnceLock;

use dyn_clone::{clone_box, DynClone};
use never::Never;
use orchid_api::atom::{Atom, Fwd, LocalAtom};
use orchid_api::expr::ExprTicket;
use orchid_api_traits::{Coding, Decode, Request};
use orchid_base::location::Pos;
use orchid_base::reqnot::Requester;
use trait_set::trait_set;

use crate::error::{ProjectError, ProjectResult};
use crate::expr::{ExprHandle, GenClause, GenExpr, OwnedExpr};
use crate::system::{atom_info_for, downcast_atom, DynSystem, DynSystemCard, SysCtx};

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
  type Info: AtomDynfo;
  const INFO: &'static Self::Info;
}
pub trait AtomicFeaturesImpl<Variant: AtomicVariant> {
  fn _factory(self) -> AtomFactory;
  type _Info: AtomDynfo;
  const _INFO: &'static Self::_Info;
}
impl<A: Atomic + AtomicFeaturesImpl<A::Variant> + ?Sized> AtomicFeatures for A {
  fn factory(self) -> AtomFactory { self._factory() }
  type Info = <Self as AtomicFeaturesImpl<A::Variant>>::_Info;
  const INFO: &'static Self::Info = Self::_INFO;
}

pub fn get_info<A: AtomCard>(sys: &(impl DynSystemCard + ?Sized)) -> (u64, &'static dyn AtomDynfo) {
  atom_info_for(sys, TypeId::of::<A>()).unwrap_or_else(|| {
    panic!("Atom {} not associated with system {}", type_name::<A>(), sys.name())
  })
}

#[derive(Clone)]
pub struct ForeignAtom {
  pub expr: ExprHandle,
  pub atom: Atom,
  pub pos: Pos,
}
impl ForeignAtom {
  pub fn oex(self) -> OwnedExpr {
    let gen_expr = GenExpr { pos: self.pos, clause: GenClause::Atom(self.expr.tk, self.atom) };
    OwnedExpr { handle: self.expr, val: OnceLock::from(Box::new(gen_expr)) }
  }
}

pub struct NotTypAtom(pub Pos, pub OwnedExpr, pub &'static dyn AtomDynfo);
impl ProjectError for NotTypAtom {
  const DESCRIPTION: &'static str = "Not the expected type";
  fn message(&self) -> String { format!("This expression is not a {}", self.2.name()) }
}

#[derive(Clone)]
pub struct TypAtom<A: AtomicFeatures> {
  pub data: ForeignAtom,
  pub value: A::Data,
}
impl<A: AtomicFeatures> TypAtom<A> {
  pub fn downcast(expr: ExprHandle) -> Result<Self, NotTypAtom> {
    match OwnedExpr::new(expr).foreign_atom() {
      Err(oe) => Err(NotTypAtom(oe.get_data().pos.clone(), oe, A::INFO)),
      Ok(atm) => match downcast_atom::<A>(atm) {
        Err(fa) => Err(NotTypAtom(fa.pos.clone(), fa.oex(), A::INFO)),
        Ok(tatom) => Ok(tatom),
      },
    }
  }
  pub fn request<R: Coding + Into<A::Req> + Request>(&self, req: R) -> R::Response {
    R::Response::decode(
      &mut &self.data.expr.ctx.reqnot.request(Fwd(self.data.atom.clone(), req.enc_vec()))[..],
    )
  }
}
impl<A: AtomicFeatures> Deref for TypAtom<A> {
  type Target = A::Data;
  fn deref(&self) -> &Self::Target { &self.value }
}

pub struct AtomCtx<'a>(pub &'a [u8], pub SysCtx);

pub trait AtomDynfo: Send + Sync + 'static {
  fn tid(&self) -> TypeId;
  fn name(&self) -> &'static str;
  fn decode(&self, ctx: AtomCtx<'_>) -> Box<dyn Any>;
  fn call(&self, ctx: AtomCtx<'_>, arg: ExprTicket) -> GenExpr;
  fn call_ref(&self, ctx: AtomCtx<'_>, arg: ExprTicket) -> GenExpr;
  fn same(&self, ctx: AtomCtx<'_>, buf2: &[u8]) -> bool;
  fn print(&self, ctx: AtomCtx<'_>) -> String;
  fn handle_req(&self, ctx: AtomCtx<'_>, req: &mut dyn Read, rep: &mut dyn Write);
  fn command(&self, ctx: AtomCtx<'_>) -> ProjectResult<Option<GenExpr>>;
  fn drop(&self, ctx: AtomCtx<'_>);
}

trait_set! {
  pub trait AtomFactoryFn = FnOnce(&dyn DynSystem) -> LocalAtom + DynClone + Send + Sync;
}
pub struct AtomFactory(Box<dyn AtomFactoryFn>);
impl AtomFactory {
  pub fn new(f: impl FnOnce(&dyn DynSystem) -> LocalAtom + Clone + Send + Sync + 'static) -> Self {
    Self(Box::new(f))
  }
  pub fn build(self, sys: &dyn DynSystem) -> LocalAtom { (self.0)(sys) }
}
impl Clone for AtomFactory {
  fn clone(&self) -> Self { AtomFactory(clone_box(&*self.0)) }
}

pub struct ErrNotCallable;
impl ProjectError for ErrNotCallable {
  const DESCRIPTION: &'static str = "This atom is not callable";
}

pub struct ErrorNotCommand;
impl ProjectError for ErrorNotCommand {
  const DESCRIPTION: &'static str = "This atom is not a command";
}

pub trait ReqPck<T: AtomCard + ?Sized>: Sized {
  type W: Write + ?Sized;
  fn unpack<'a>(self) -> (T::Req, &'a mut Self::W)
  where Self: 'a;
  fn never(self)
  where T: AtomCard<Req = Never> {
  }
}

pub struct RequestPack<'a, T: AtomCard + ?Sized, W: Write + ?Sized>(pub T::Req, pub &'a mut W);

impl<'a, T: AtomCard + ?Sized, W: Write + ?Sized> ReqPck<T> for RequestPack<'a, T, W> {
  type W = W;
  fn unpack<'b>(self) -> (<T as AtomCard>::Req, &'b mut Self::W)
  where 'a: 'b {
    (self.0, self.1)
  }
}
