use std::fmt;
use std::ops::Deref;
use std::sync::{Arc, OnceLock};

use derive_destructure::destructure;
use orchid_api::InspectedKind;
use orchid_base::error::{OrcErr, OrcErrv};
use orchid_base::interner::Tok;
use orchid_base::location::Pos;
use orchid_base::reqnot::Requester;

use crate::api;
use crate::atom::{AtomFactory, ForeignAtom, ToAtom};
use crate::system::SysCtx;

#[derive(destructure)]
pub struct ExprHandle {
  pub tk: api::ExprTicket,
  pub ctx: SysCtx,
}
impl ExprHandle {
  pub(crate) fn from_args(ctx: SysCtx, tk: api::ExprTicket) -> Self { Self { ctx, tk } }
  pub fn get_ctx(&self) -> SysCtx { self.ctx.clone() }
}
impl fmt::Debug for ExprHandle {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "ExprHandle({})", self.tk.0)
  }
}
impl Clone for ExprHandle {
  fn clone(&self) -> Self {
    self.ctx.reqnot.notify(api::Acquire(self.ctx.id, self.tk));
    Self { ctx: self.ctx.clone(), tk: self.tk }
  }
}
impl Drop for ExprHandle {
  fn drop(&mut self) { self.ctx.reqnot.notify(api::Release(self.ctx.id, self.tk)) }
}

#[derive(Clone, Debug, destructure)]
pub struct Expr {
  pub handle: Option<Arc<ExprHandle>>,
  pub val: OnceLock<ExprData>,
}
impl Expr {
  pub fn new(hand: Arc<ExprHandle>) -> Self { Self { handle: Some(hand), val: OnceLock::new() } }
  pub fn from_data(val: ExprData) -> Self { Self { handle: None, val: OnceLock::from(val) } }
  pub fn get_data(&self) -> &ExprData {
    self.val.get_or_init(|| {
      let handle = self.handle.as_ref().expect("Either the value or the handle must be set");
      let details = handle.ctx.reqnot.request(api::Inspect { target: handle.tk });
      let pos = Pos::from_api(&details.location);
      let kind = match details.kind {
        InspectedKind::Atom(a) => ExprKind::Atom(ForeignAtom::new(handle.clone(), a, pos.clone())),
        InspectedKind::Bottom(b) => ExprKind::Bottom(OrcErrv::from_api(&b)),
        InspectedKind::Opaque => ExprKind::Opaque,
      };
      ExprData { pos, kind }
    })
  }
  pub fn foreign_atom(self) -> Result<ForeignAtom<'static>, Self> {
    match (self.get_data(), &self.handle) {
      (ExprData { kind: ExprKind::Atom(atom), .. }, Some(_)) => Ok(atom.clone()),
      _ => Err(self),
    }
  }
  pub fn api_return(
    self,
    ctx: SysCtx,
    do_slot: &mut impl FnMut(Arc<ExprHandle>),
  ) -> api::Expression {
    if let Some(h) = self.handle {
      do_slot(h.clone());
      api::Expression { location: api::Location::SlotTarget, kind: api::ExpressionKind::Slot(h.tk) }
    } else {
      self.val.into_inner().expect("Either value or handle must be set").api_return(ctx, do_slot)
    }
  }
  pub fn handle(&self) -> Option<Arc<ExprHandle>> { self.handle.clone() }
}
impl Deref for Expr {
  type Target = ExprData;
  fn deref(&self) -> &Self::Target { self.get_data() }
}

#[derive(Clone, Debug)]
pub struct ExprData {
  pub pos: Pos,
  pub kind: ExprKind,
}
impl ExprData {
  pub fn api_return(
    self,
    ctx: SysCtx,
    do_slot: &mut impl FnMut(Arc<ExprHandle>),
  ) -> api::Expression {
    api::Expression { location: self.pos.to_api(), kind: self.kind.api_return(ctx, do_slot) }
  }
}

#[derive(Clone, Debug)]
pub enum ExprKind {
  Call(Box<Expr>, Box<Expr>),
  Lambda(u64, Box<Expr>),
  Arg(u64),
  Seq(Box<Expr>, Box<Expr>),
  Const(Tok<Vec<Tok<String>>>),
  NewAtom(AtomFactory),
  Atom(ForeignAtom<'static>),
  Bottom(OrcErrv),
  Opaque,
}
impl ExprKind {
  pub fn api_return(
    self,
    ctx: SysCtx,
    do_slot: &mut impl FnMut(Arc<ExprHandle>),
  ) -> api::ExpressionKind {
    use api::ExpressionKind as K;
    match self {
      Self::Call(f, x) =>
        K::Call(Box::new(f.api_return(ctx.clone(), do_slot)), Box::new(x.api_return(ctx, do_slot))),
      Self::Seq(a, b) =>
        K::Seq(Box::new(a.api_return(ctx.clone(), do_slot)), Box::new(b.api_return(ctx, do_slot))),
      Self::Lambda(arg, body) => K::Lambda(arg, Box::new(body.api_return(ctx, do_slot))),
      Self::Arg(arg) => K::Arg(arg),
      Self::Const(name) => K::Const(name.marker()),
      Self::Bottom(err) => K::Bottom(err.to_api()),
      Self::NewAtom(fac) => K::NewAtom(fac.clone().build(ctx)),
      kind @ (Self::Atom(_) | Self::Opaque) => panic!("{kind:?} should have a token"),
    }
  }
}
fn inherit(kind: ExprKind) -> Expr { Expr::from_data(ExprData { pos: Pos::Inherit, kind }) }

pub fn sym_ref(path: Tok<Vec<Tok<String>>>) -> Expr { inherit(ExprKind::Const(path)) }
pub fn atom<A: ToAtom>(atom: A) -> Expr { inherit(ExprKind::NewAtom(atom.to_atom_factory())) }

pub fn seq(ops: impl IntoIterator<Item = Expr>) -> Expr {
  fn recur(mut ops: impl Iterator<Item = Expr>) -> Option<Expr> {
    let op = ops.next()?;
    Some(match recur(ops) {
      None => op,
      Some(rec) => inherit(ExprKind::Seq(Box::new(op), Box::new(rec))),
    })
  }
  recur(ops.into_iter()).expect("Empty list provided to seq!")
}

pub fn arg(n: u64) -> ExprKind { ExprKind::Arg(n) }

pub fn lambda(n: u64, b: impl IntoIterator<Item = Expr>) -> Expr {
  inherit(ExprKind::Lambda(n, Box::new(call(b))))
}

pub fn call(v: impl IntoIterator<Item = Expr>) -> Expr {
  v.into_iter()
    .reduce(|f, x| inherit(ExprKind::Call(Box::new(f), Box::new(x))))
    .expect("Empty call expression")
}

pub fn bot(ev: impl IntoIterator<Item = OrcErr>) -> Expr {
  inherit(ExprKind::Bottom(OrcErrv::new(ev).unwrap()))
}
