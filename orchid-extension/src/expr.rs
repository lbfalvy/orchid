use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::OnceLock;

use derive_destructure::destructure;
use orchid_base::error::{errv_from_apiv, errv_to_apiv, OrcErr};
use orchid_base::interner::{deintern, Tok};
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
  pub(crate) fn into_tk(self) -> api::ExprTicket {
    let (tk, ..) = self.destructure();
    tk
  }
  pub fn get_ctx(&self) -> SysCtx { self.ctx.clone() }
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

#[derive(Clone, destructure)]
pub struct OwnedExpr {
  pub handle: ExprHandle,
  pub val: OnceLock<Box<GenExpr>>,
}
impl OwnedExpr {
  pub fn new(handle: ExprHandle) -> Self { Self { handle, val: OnceLock::new() } }
  pub fn get_data(&self) -> &GenExpr {
    self.val.get_or_init(|| {
      Box::new(GenExpr::from_api(
        self.handle.ctx.reqnot.request(api::Inspect(self.handle.tk)).expr,
        &self.handle.ctx,
      ))
    })
  }
  pub fn foreign_atom(self) -> Result<ForeignAtom<'static>, Self> {
    if let GenExpr { clause: GenClause::Atom(_, atom), pos: position } = self.get_data() {
      let (atom, position) = (atom.clone(), position.clone());
      return Ok(ForeignAtom {
        ctx: self.handle.ctx.clone(),
        expr: Some(self.handle),
        char_marker: PhantomData,
        pos: position,
        atom,
      });
    }
    Err(self)
  }
}
impl Deref for OwnedExpr {
  type Target = GenExpr;
  fn deref(&self) -> &Self::Target { self.get_data() }
}

#[derive(Clone)]
pub struct GenExpr {
  pub pos: Pos,
  pub clause: GenClause,
}
impl GenExpr {
  pub fn to_api(&self, ctx: SysCtx) -> api::Expr {
    api::Expr { location: self.pos.to_api(), clause: self.clause.to_api(ctx) }
  }
  pub fn into_api(self, ctx: SysCtx) -> api::Expr {
    api::Expr { location: self.pos.to_api(), clause: self.clause.into_api(ctx) }
  }
  pub fn from_api(api: api::Expr, ctx: &SysCtx) -> Self {
    Self { pos: Pos::from_api(&api.location), clause: GenClause::from_api(api.clause, ctx) }
  }
}

#[derive(Clone)]
pub enum GenClause {
  Call(Box<GenExpr>, Box<GenExpr>),
  Lambda(u64, Box<GenExpr>),
  Arg(u64),
  Slot(OwnedExpr),
  Seq(Box<GenExpr>, Box<GenExpr>),
  Const(Tok<Vec<Tok<String>>>),
  NewAtom(AtomFactory),
  Atom(api::ExprTicket, api::Atom),
  Bottom(Vec<OrcErr>),
}
impl GenClause {
  pub fn to_api(&self, ctx: SysCtx) -> api::Clause {
    match self {
      Self::Call(f, x) =>
        api::Clause::Call(Box::new(f.to_api(ctx.clone())), Box::new(x.to_api(ctx))),
      Self::Seq(a, b) => api::Clause::Seq(Box::new(a.to_api(ctx.clone())), Box::new(b.to_api(ctx))),
      Self::Lambda(arg, body) => api::Clause::Lambda(*arg, Box::new(body.to_api(ctx))),
      Self::Arg(arg) => api::Clause::Arg(*arg),
      Self::Const(name) => api::Clause::Const(name.marker()),
      Self::Bottom(err) => api::Clause::Bottom(errv_to_apiv(err)),
      Self::NewAtom(fac) => api::Clause::NewAtom(fac.clone().build(ctx)),
      Self::Atom(tk, atom) => api::Clause::Atom(*tk, atom.clone()),
      Self::Slot(_) => panic!("Slot is forbidden in const tree"),
    }
  }
  pub fn into_api(self, ctx: SysCtx) -> api::Clause {
    match self {
      Self::Call(f, x) =>
        api::Clause::Call(Box::new(f.into_api(ctx.clone())), Box::new(x.into_api(ctx))),
      Self::Seq(a, b) =>
        api::Clause::Seq(Box::new(a.into_api(ctx.clone())), Box::new(b.into_api(ctx))),
      Self::Lambda(arg, body) => api::Clause::Lambda(arg, Box::new(body.into_api(ctx))),
      Self::Arg(arg) => api::Clause::Arg(arg),
      Self::Slot(extk) => api::Clause::Slot(extk.handle.into_tk()),
      Self::Const(name) => api::Clause::Const(name.marker()),
      Self::Bottom(err) => api::Clause::Bottom(errv_to_apiv(err.iter())),
      Self::NewAtom(fac) => api::Clause::NewAtom(fac.clone().build(ctx)),
      Self::Atom(tk, atom) => api::Clause::Atom(tk, atom),
    }
  }
  pub fn from_api(api: api::Clause, ctx: &SysCtx) -> Self {
    match api {
      api::Clause::Arg(id) => Self::Arg(id),
      api::Clause::Lambda(arg, body) => Self::Lambda(arg, Box::new(GenExpr::from_api(*body, ctx))),
      api::Clause::NewAtom(_) => panic!("Clause::NewAtom should never be received, only sent"),
      api::Clause::Bottom(s) => Self::Bottom(errv_from_apiv(&s)),
      api::Clause::Call(f, x) =>
        Self::Call(Box::new(GenExpr::from_api(*f, ctx)), Box::new(GenExpr::from_api(*x, ctx))),
      api::Clause::Seq(a, b) =>
        Self::Seq(Box::new(GenExpr::from_api(*a, ctx)), Box::new(GenExpr::from_api(*b, ctx))),
      api::Clause::Const(name) => Self::Const(deintern(name)),
      api::Clause::Slot(exi) => Self::Slot(OwnedExpr::new(ExprHandle::from_args(ctx.clone(), exi))),
      api::Clause::Atom(tk, atom) => Self::Atom(tk, atom),
    }
  }
}
fn inherit(clause: GenClause) -> GenExpr { GenExpr { pos: Pos::Inherit, clause } }

pub fn sym_ref(path: Tok<Vec<Tok<String>>>) -> GenExpr { inherit(GenClause::Const(path)) }
pub fn atom<A: ToAtom>(atom: A) -> GenExpr { inherit(GenClause::NewAtom(atom.to_atom_factory())) }

pub fn seq(ops: impl IntoIterator<Item = GenExpr>) -> GenExpr {
  fn recur(mut ops: impl Iterator<Item = GenExpr>) -> Option<GenExpr> {
    let op = ops.next()?;
    Some(match recur(ops) {
      None => op,
      Some(rec) => inherit(GenClause::Seq(Box::new(op), Box::new(rec))),
    })
  }
  recur(ops.into_iter()).expect("Empty list provided to seq!")
}

pub fn slot(extk: OwnedExpr) -> GenClause { GenClause::Slot(extk) }

pub fn arg(n: u64) -> GenClause { GenClause::Arg(n) }

pub fn lambda(n: u64, b: impl IntoIterator<Item = GenExpr>) -> GenExpr {
  inherit(GenClause::Lambda(n, Box::new(call(b))))
}

pub fn call(v: impl IntoIterator<Item = GenExpr>) -> GenExpr {
  v.into_iter()
    .reduce(|f, x| inherit(GenClause::Call(Box::new(f), Box::new(x))))
    .expect("Empty call expression")
}

pub fn bot(e: OrcErr) -> GenExpr { botv(vec![e]) }
pub fn botv(ev: Vec<OrcErr>) -> GenExpr { inherit(GenClause::Bottom(ev)) }
