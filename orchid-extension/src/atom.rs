use std::any::{type_name, Any, TypeId};
use std::fmt;
use std::io::{Read, Write};
use std::marker::PhantomData;
use std::ops::{Deref, Range};
use std::sync::OnceLock;

use dyn_clone::{clone_box, DynClone};
use never::Never;
use orchid_api::ExprTicket;
use orchid_api_traits::{enc_vec, Coding, Decode, Request};
use orchid_base::error::{mk_err, OrcErr, OrcRes};
use orchid_base::intern;
use orchid_base::location::Pos;
use orchid_base::reqnot::Requester;
use orchid_base::tree::AtomInTok;
use trait_set::trait_set;

use crate::api;
// use crate::error::{ProjectError, ProjectResult};
use crate::expr::{ExprHandle, GenClause, GenExpr, OwnedExpr};
use crate::system::{atom_info_for, downcast_atom, DynSystemCard, SysCtx};

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
pub trait ToAtom {
  fn to_atom_factory(self) -> AtomFactory;
}
impl<A: AtomicFeatures> ToAtom for A {
  fn to_atom_factory(self) -> AtomFactory { self.factory() }
}
impl ToAtom for AtomFactory {
  fn to_atom_factory(self) -> AtomFactory { self }
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

pub fn get_info<A: AtomCard>(
  sys: &(impl DynSystemCard + ?Sized),
) -> (api::AtomId, &'static dyn AtomDynfo) {
  atom_info_for(sys, TypeId::of::<A>()).unwrap_or_else(|| {
    panic!("Atom {} not associated with system {}", type_name::<A>(), sys.name())
  })
}

#[derive(Clone)]
pub struct ForeignAtom<'a> {
  pub expr: Option<ExprHandle>,
  pub char_marker: PhantomData<&'a ()>,
  pub ctx: SysCtx,
  pub atom: api::Atom,
  pub pos: Pos,
}
impl<'a> ForeignAtom<'a> {
  pub fn oex_opt(self) -> Option<OwnedExpr> {
    self.expr.map(|handle| {
      let gen_expr = GenExpr { pos: self.pos, clause: GenClause::Atom(handle.tk, self.atom) };
      OwnedExpr { handle, val: OnceLock::from(Box::new(gen_expr)) }
    })
  }
}
impl ForeignAtom<'static> {
  pub fn oex(self) -> OwnedExpr { self.oex_opt().unwrap() }
}
impl<'a> fmt::Display for ForeignAtom<'a> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}::{:?}", if self.expr.is_some() { "Clause" } else { "Tok" }, self.atom)
  }
}
impl<'a> AtomInTok for ForeignAtom<'a> {
  type Context = SysCtx;
  fn from_api(atom: &api::Atom, pos: Range<u32>, ctx: &mut Self::Context) -> Self {
    Self {
      atom: atom.clone(),
      char_marker: PhantomData,
      ctx: ctx.clone(),
      expr: None,
      pos: Pos::Range(pos),
    }
  }
  fn to_api(&self) -> orchid_api::Atom { self.atom.clone() }
}

pub struct NotTypAtom(pub Pos, pub OwnedExpr, pub &'static dyn AtomDynfo);
impl NotTypAtom {
  pub fn mk_err(&self) -> OrcErr {
    mk_err(
      intern!(str: "Not the expected type"),
      format!("This expression is not a {}", self.2.name()),
      [self.0.clone().into()],
    )
  }
}

#[derive(Clone)]
pub struct TypAtom<'a, A: AtomicFeatures> {
  pub data: ForeignAtom<'a>,
  pub value: A::Data,
}
impl<A: AtomicFeatures> TypAtom<'static, A> {
  pub fn downcast(expr: ExprHandle) -> Result<Self, NotTypAtom> {
    match OwnedExpr::new(expr).foreign_atom() {
      Err(oe) => Err(NotTypAtom(oe.get_data().pos.clone(), oe, A::INFO)),
      Ok(atm) => match downcast_atom::<A>(atm) {
        Err(fa) => Err(NotTypAtom(fa.pos.clone(), fa.oex(), A::INFO)),
        Ok(tatom) => Ok(tatom),
      },
    }
  }
}
impl<'a, A: AtomicFeatures> TypAtom<'a, A> {
  pub fn request<R: Coding + Into<A::Req> + Request>(&self, req: R) -> R::Response {
    R::Response::decode(
      &mut &self.data.ctx.reqnot.request(api::Fwd(self.data.atom.clone(), enc_vec(&req)))[..],
    )
  }
}
impl<'a, A: AtomicFeatures> Deref for TypAtom<'a, A> {
  type Target = A::Data;
  fn deref(&self) -> &Self::Target { &self.value }
}

pub struct AtomCtx<'a>(pub &'a [u8], pub Option<api::AtomId>, pub SysCtx);

pub trait AtomDynfo: Send + Sync + 'static {
  fn tid(&self) -> TypeId;
  fn name(&self) -> &'static str;
  fn decode(&self, ctx: AtomCtx<'_>) -> Box<dyn Any>;
  fn call(&self, ctx: AtomCtx<'_>, arg: api::ExprTicket) -> GenExpr;
  fn call_ref(&self, ctx: AtomCtx<'_>, arg: api::ExprTicket) -> GenExpr;
  fn same(&self, ctx: AtomCtx<'_>, other: &api::Atom) -> bool;
  fn print(&self, ctx: AtomCtx<'_>) -> String;
  fn handle_req(&self, ctx: AtomCtx<'_>, req: &mut dyn Read, rep: &mut dyn Write);
  fn command(&self, ctx: AtomCtx<'_>) -> OrcRes<Option<GenExpr>>;
  fn serialize(&self, ctx: AtomCtx<'_>, write: &mut dyn Write) -> Vec<ExprTicket>;
  fn deserialize(&self, ctx: SysCtx, data: &[u8], refs: &[ExprTicket]) -> api::Atom;
  fn drop(&self, ctx: AtomCtx<'_>);
}

trait_set! {
  pub trait AtomFactoryFn = FnOnce(SysCtx) -> api::Atom + DynClone + Send + Sync;
}
pub struct AtomFactory(Box<dyn AtomFactoryFn>);
impl AtomFactory {
  pub fn new(f: impl FnOnce(SysCtx) -> api::Atom + Clone + Send + Sync + 'static) -> Self {
    Self(Box::new(f))
  }
  pub fn build(self, ctx: SysCtx) -> api::Atom { (self.0)(ctx) }
}
impl Clone for AtomFactory {
  fn clone(&self) -> Self { AtomFactory(clone_box(&*self.0)) }
}

pub fn err_not_callable() -> OrcErr {
  mk_err(intern!(str: "This atom is not callable"), "Attempted to apply value as function", [])
}

pub fn err_not_command() -> OrcErr {
  mk_err(intern!(str: "This atom is not a command"), "Settled on an inactionable value", [])
}

pub trait ReqPck<T: AtomCard + ?Sized>: Sized {
  type W: Write + ?Sized;
  fn unpack<'a>(self) -> (T::Req, &'a mut Self::W, SysCtx)
  where Self: 'a;
  fn never(self)
  where T: AtomCard<Req = Never> {
  }
}

pub(crate) struct RequestPack<'a, T: AtomCard + ?Sized, W: Write + ?Sized> {
  pub req: T::Req,
  pub write: &'a mut W,
  pub sys: SysCtx,
}

impl<'a, T: AtomCard + ?Sized, W: Write + ?Sized> ReqPck<T> for RequestPack<'a, T, W> {
  type W = W;
  fn unpack<'b>(self) -> (<T as AtomCard>::Req, &'b mut Self::W, SysCtx)
  where 'a: 'b {
    (self.req, self.write, self.sys)
  }
}
