use std::any::{type_name, Any, TypeId};
use std::fmt;
use std::io::{Read, Write};
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::{Arc, OnceLock};

use dyn_clone::{clone_box, DynClone};
use orchid_api_traits::{enc_vec, Coding, Decode, Encode, Request};
use orchid_base::error::{mk_err, OrcErr, OrcRes};
use orchid_base::intern;
use orchid_base::location::Pos;
use orchid_base::name::Sym;
use orchid_base::reqnot::Requester;
use orchid_base::tree::AtomRepr;
use trait_set::trait_set;

use crate::api;
// use crate::error::{ProjectError, ProjectResult};
use crate::expr::{Expr, ExprData, ExprHandle, ExprKind};
use crate::system::{atom_info_for, downcast_atom, DynSystemCard, SysCtx};

pub trait AtomCard: 'static + Sized {
  type Data: Clone + Coding + Sized;
}

pub trait AtomicVariant {}
pub trait Atomic: 'static + Sized {
  type Variant: AtomicVariant;
  type Data: Clone + Coding + Sized;
  fn reg_reqs() -> MethodSet<Self>;
}
impl<A: Atomic> AtomCard for A {
  type Data = <Self as Atomic>::Data;
}

pub trait AtomicFeatures: Atomic {
  fn factory(self) -> AtomFactory;
  type Info: AtomDynfo;
  fn info() -> Self::Info;
  fn dynfo() -> Box<dyn AtomDynfo>;
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
  fn _info() -> Self::_Info;
}
impl<A: Atomic + AtomicFeaturesImpl<A::Variant>> AtomicFeatures for A {
  fn factory(self) -> AtomFactory { self._factory() }
  type Info = <Self as AtomicFeaturesImpl<A::Variant>>::_Info;
  fn info() -> Self::Info { Self::_info() }
  fn dynfo() -> Box<dyn AtomDynfo> { Box::new(Self::info()) }
}

pub fn get_info<A: AtomCard>(
  sys: &(impl DynSystemCard + ?Sized),
) -> (api::AtomId, Box<dyn AtomDynfo>) {
  atom_info_for(sys, TypeId::of::<A>()).unwrap_or_else(|| {
    panic!("Atom {} not associated with system {}", type_name::<A>(), sys.name())
  })
}

#[derive(Clone)]
pub struct ForeignAtom<'a> {
  pub expr: Option<Arc<ExprHandle>>,
  pub _life: PhantomData<&'a ()>,
  pub ctx: SysCtx,
  pub atom: api::Atom,
  pub pos: Pos,
}
impl ForeignAtom<'_> {
  pub fn oex_opt(self) -> Option<Expr> {
    let (handle, pos) = (self.expr.as_ref()?.clone(), self.pos.clone());
    let data = ExprData { pos, kind: ExprKind::Atom(ForeignAtom { _life: PhantomData, ..self }) };
    Some(Expr { handle: Some(handle), val: OnceLock::from(data) })
  }
}
impl ForeignAtom<'static> {
  pub fn oex(self) -> Expr { self.oex_opt().unwrap() }
  pub(crate) fn new(handle: Arc<ExprHandle>, atom: api::Atom, pos: Pos) -> Self {
    ForeignAtom { _life: PhantomData, atom, ctx: handle.ctx.clone(), expr: Some(handle), pos }
  }
  pub fn request<M: AtomMethod>(&self, m: M) -> Option<M::Response> {
    let rep = self.ctx.reqnot.request(api::Fwd(
      self.atom.clone(),
      Sym::parse(M::NAME).unwrap().tok().to_api(),
      enc_vec(&m)
    ))?;
    Some(M::Response::decode(&mut &rep[..]))
  }
}
impl fmt::Display for ForeignAtom<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}::{:?}", if self.expr.is_some() { "Clause" } else { "Tok" }, self.atom)
  }
}
impl fmt::Debug for ForeignAtom<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "ForeignAtom({self})") }
}
impl AtomRepr for ForeignAtom<'_> {
  type Ctx = SysCtx;
  fn from_api(atom: &api::Atom, pos: Pos, ctx: &mut Self::Ctx) -> Self {
    Self { atom: atom.clone(), _life: PhantomData, ctx: ctx.clone(), expr: None, pos }
  }
  fn to_api(&self) -> orchid_api::Atom { self.atom.clone() }
}

pub struct NotTypAtom(pub Pos, pub Expr, pub Box<dyn AtomDynfo>);
impl NotTypAtom {
  pub fn mk_err(&self) -> OrcErr {
    mk_err(
      intern!(str: "Not the expected type"),
      format!("This expression is not a {}", self.2.name()),
      [self.0.clone().into()],
    )
  }
}

pub trait AtomMethod: Request {
  const NAME: &str;
}
pub trait Supports<M: AtomMethod>: AtomCard {
  fn handle(&self, ctx: SysCtx, req: M) -> <M as Request>::Response;
}

trait_set! {
  trait AtomReqCb<A> = Fn(&A, SysCtx, &mut dyn Read, &mut dyn Write) + Send + Sync
}

pub struct AtomReqHandler<A: AtomCard> {
  key: Sym,
  cb: Box<dyn AtomReqCb<A>>,
}

pub struct MethodSet<A: AtomCard> {
  handlers: Vec<AtomReqHandler<A>>,
}
impl<A: AtomCard> MethodSet<A> {
  pub fn new() -> Self { Self{ handlers: vec![] } }

  pub fn handle<M: AtomMethod>(mut self) -> Self where A: Supports<M> {
    self.handlers.push(AtomReqHandler {
      key: Sym::parse(M::NAME).expect("AtomMethod::NAME cannoot be empty"),
      cb: Box::new(move |
        a: &A,
        ctx: SysCtx,
        req: &mut dyn Read,
        rep: &mut dyn Write
      | {
        Supports::<M>::handle(a, ctx, M::decode(req)).encode(rep);
      })
    });
    self
  }

  pub(crate) fn dispatch(
    &self, atom: &A, ctx: SysCtx, key: Sym, req: &mut dyn Read, rep: &mut dyn Write
  ) -> bool {
    match self.handlers.iter().find(|h| h.key == key) {
      None => false,
      Some(handler) => {
        (handler.cb)(atom, ctx, req, rep);
        true
      },
    }
  }
}

impl<A: AtomCard> Default for MethodSet<A> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct TypAtom<'a, A: AtomicFeatures> {
  pub data: ForeignAtom<'a>,
  pub value: A::Data,
}
impl<A: AtomicFeatures> TypAtom<'static, A> {
  pub fn downcast(expr: Arc<ExprHandle>) -> Result<Self, NotTypAtom> {
    match Expr::new(expr).foreign_atom() {
      Err(oe) => Err(NotTypAtom(oe.get_data().pos.clone(), oe, Box::new(A::info()))),
      Ok(atm) => match downcast_atom::<A>(atm) {
        Err(fa) => Err(NotTypAtom(fa.pos.clone(), fa.oex(), Box::new(A::info()))),
        Ok(tatom) => Ok(tatom),
      },
    }
  }
}
impl<A: AtomicFeatures> TypAtom<'_, A> {
  pub fn request<M: AtomMethod>(&self, req: M) -> M::Response where A: Supports<M> {
    M::Response::decode(
      &mut &self.data.ctx.reqnot.request(api::Fwd(
        self.data.atom.clone(),
        Sym::parse(M::NAME).unwrap().tok().to_api(),
        enc_vec(&req)
      )).unwrap()[..]
    )
  }
}
impl<A: AtomicFeatures> Deref for TypAtom<'_, A> {
  type Target = A::Data;
  fn deref(&self) -> &Self::Target { &self.value }
}

pub struct AtomCtx<'a>(pub &'a [u8], pub Option<api::AtomId>, pub SysCtx);

pub trait AtomDynfo: Send + Sync + 'static {
  fn tid(&self) -> TypeId;
  fn name(&self) -> &'static str;
  fn decode(&self, ctx: AtomCtx<'_>) -> Box<dyn Any>;
  fn call(&self, ctx: AtomCtx<'_>, arg: api::ExprTicket) -> Expr;
  fn call_ref(&self, ctx: AtomCtx<'_>, arg: api::ExprTicket) -> Expr;
  fn print(&self, ctx: AtomCtx<'_>) -> String;
  fn handle_req(&self, ctx: AtomCtx<'_>, key: Sym, req: &mut dyn Read, rep: &mut dyn Write) -> bool;
  fn command(&self, ctx: AtomCtx<'_>) -> OrcRes<Option<Expr>>;
  fn serialize(&self, ctx: AtomCtx<'_>, write: &mut dyn Write) -> Option<Vec<api::ExprTicket>>;
  fn deserialize(&self, ctx: SysCtx, data: &[u8], refs: &[api::ExprTicket]) -> api::Atom;
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
impl fmt::Debug for AtomFactory {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "AtomFactory") }
}
impl fmt::Display for AtomFactory {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "AtomFactory") }
}

pub fn err_not_callable() -> OrcErr {
  mk_err(intern!(str: "This atom is not callable"), "Attempted to apply value as function", [])
}

pub fn err_not_command() -> OrcErr {
  mk_err(intern!(str: "This atom is not a command"), "Settled on an inactionable value", [])
}
