use std::ops::Deref;
use std::sync::{Arc, OnceLock};

use derive_destructure::destructure;
use orchid_api::atom::Atom;
use orchid_api::expr::{Acquire, Clause, Expr, ExprTicket, Inspect, Release};
use orchid_base::interner::{deintern, Tok};
use orchid_base::location::Pos;
use orchid_base::reqnot::Requester;

use crate::atom::{AtomFactory, AtomicFeatures, ForeignAtom};
use crate::error::{err_from_apiv, errv_to_apiv, DynProjectError, ProjectErrorObj};
use crate::system::{DynSystem, SysCtx};

#[derive(destructure)]
pub struct ExprHandle {
  pub tk: ExprTicket,
  pub ctx: SysCtx,
}
impl ExprHandle {
  pub(crate) fn from_args(ctx: SysCtx, tk: ExprTicket) -> Self { Self { ctx, tk } }
  pub(crate) fn into_tk(self) -> ExprTicket {
    let (tk, ..) = self.destructure();
    tk
  }
  pub fn get_ctx(&self) -> SysCtx { self.ctx.clone() }
}
impl Clone for ExprHandle {
  fn clone(&self) -> Self {
    self.ctx.reqnot.notify(Acquire(self.ctx.id, self.tk));
    Self { ctx: self.ctx.clone(), tk: self.tk }
  }
}
impl Drop for ExprHandle {
  fn drop(&mut self) { self.ctx.reqnot.notify(Release(self.ctx.id, self.tk)) }
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
        self.handle.ctx.reqnot.request(Inspect(self.handle.tk)).expr,
        &self.handle.ctx,
      ))
    })
  }
  pub fn foreign_atom(self) -> Result<ForeignAtom, Self> {
    if let GenExpr { clause: GenClause::Atom(_, atom), pos: position } = self.get_data() {
      let (atom, position) = (atom.clone(), position.clone());
      return Ok(ForeignAtom { expr: self.handle, atom, pos: position });
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
  pub fn to_api(&self, sys: &dyn DynSystem) -> Expr {
    Expr { location: self.pos.to_api(), clause: self.clause.to_api(sys) }
  }
  pub fn into_api(self, sys: &dyn DynSystem) -> Expr {
    Expr { location: self.pos.to_api(), clause: self.clause.into_api(sys) }
  }
  pub fn from_api(api: Expr, ctx: &SysCtx) -> Self {
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
  Atom(ExprTicket, Atom),
  Bottom(ProjectErrorObj),
}
impl GenClause {
  pub fn to_api(&self, sys: &dyn DynSystem) -> Clause {
    match self {
      Self::Call(f, x) => Clause::Call(Box::new(f.to_api(sys)), Box::new(x.to_api(sys))),
      Self::Seq(a, b) => Clause::Seq(Box::new(a.to_api(sys)), Box::new(b.to_api(sys))),
      Self::Lambda(arg, body) => Clause::Lambda(*arg, Box::new(body.to_api(sys))),
      Self::Arg(arg) => Clause::Arg(*arg),
      Self::Const(name) => Clause::Const(name.marker()),
      Self::Bottom(err) => Clause::Bottom(errv_to_apiv([err.clone()])),
      Self::NewAtom(fac) => Clause::NewAtom(fac.clone().build(sys)),
      Self::Atom(tk, atom) => Clause::Atom(*tk, atom.clone()),
      Self::Slot(_) => panic!("Slot is forbidden in const tree"),
    }
  }
  pub fn into_api(self, sys: &dyn DynSystem) -> Clause {
    match self {
      Self::Call(f, x) => Clause::Call(Box::new(f.into_api(sys)), Box::new(x.into_api(sys))),
      Self::Seq(a, b) => Clause::Seq(Box::new(a.into_api(sys)), Box::new(b.into_api(sys))),
      Self::Lambda(arg, body) => Clause::Lambda(arg, Box::new(body.into_api(sys))),
      Self::Arg(arg) => Clause::Arg(arg),
      Self::Slot(extk) => Clause::Slot(extk.handle.into_tk()),
      Self::Const(name) => Clause::Const(name.marker()),
      Self::Bottom(err) => Clause::Bottom(errv_to_apiv([err])),
      Self::NewAtom(fac) => Clause::NewAtom(fac.clone().build(sys)),
      Self::Atom(tk, atom) => Clause::Atom(tk, atom),
    }
  }
  pub fn from_api(api: Clause, ctx: &SysCtx) -> Self {
    match api {
      Clause::Arg(id) => Self::Arg(id),
      Clause::Lambda(arg, body) => Self::Lambda(arg, Box::new(GenExpr::from_api(*body, ctx))),
      Clause::NewAtom(_) => panic!("Clause::NewAtom should never be received, only sent"),
      Clause::Bottom(s) => Self::Bottom(err_from_apiv(&s, &ctx.reqnot)),
      Clause::Call(f, x) => Self::Call(
        Box::new(GenExpr::from_api(*f, ctx)),
        Box::new(GenExpr::from_api(*x, ctx)),
      ),
      Clause::Seq(a, b) => Self::Seq(
        Box::new(GenExpr::from_api(*a, ctx)),
        Box::new(GenExpr::from_api(*b, ctx)),
      ),
      Clause::Const(name) => Self::Const(deintern(name)),
      Clause::Slot(exi) => Self::Slot(OwnedExpr::new(ExprHandle::from_args(ctx.clone(), exi))),
      Clause::Atom(tk, atom) => Self::Atom(tk, atom),
    }
  }
}
fn inherit(clause: GenClause) -> GenExpr { GenExpr { pos: Pos::Inherit, clause } }

pub fn sym_ref(path: Tok<Vec<Tok<String>>>) -> GenExpr { inherit(GenClause::Const(path)) }
pub fn atom<A: AtomicFeatures>(atom: A) -> GenExpr { inherit(GenClause::NewAtom(atom.factory())) }

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

pub fn bot<E: DynProjectError>(msg: E) -> GenExpr { inherit(GenClause::Bottom(Arc::new(msg))) }
pub fn bot_obj(e: ProjectErrorObj) -> GenExpr { inherit(GenClause::Bottom(e)) }
