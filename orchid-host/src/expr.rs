use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use hashbrown::HashMap;
use lazy_static::lazy_static;
use orchid_base::error::OrcErrv;
use orchid_base::interner::deintern;
use orchid_base::location::Pos;
use orchid_base::name::Sym;
use orchid_base::tree::AtomTok;

use crate::api;
use crate::extension::AtomHand;

pub type ExprParseCtx = ();

#[derive(Clone, Debug)]
pub struct Expr {
  is_canonical: Arc<AtomicBool>,
  pos: Pos,
  kind: Arc<RwLock<ExprKind>>,
}
impl Expr {
  pub fn pos(&self) -> Pos { self.pos.clone() }
  pub fn as_atom(&self) -> Option<AtomHand> { todo!() }
  pub fn strong_count(&self) -> usize { todo!() }
  pub fn id(&self) -> api::ExprTicket {
    api::ExprTicket(
      NonZeroU64::new(self.kind.as_ref() as *const RwLock<_> as usize as u64)
        .expect("this is a ref, it cannot be null"),
    )
  }
  pub fn canonicalize(&self) -> api::ExprTicket {
    if !self.is_canonical.swap(true, Ordering::Relaxed) {
      KNOWN_EXPRS.write().unwrap().entry(self.id()).or_insert_with(|| self.clone());
    }
    self.id()
  }
  pub fn resolve(tk: api::ExprTicket) -> Option<Self> {
    KNOWN_EXPRS.read().unwrap().get(&tk).cloned()
  }
  pub fn from_api(api: api::Expression, ctx: &mut ExprParseCtx) -> Self {
    if let api::ExpressionKind::Slot(tk) = &api.kind {
      return Self::resolve(*tk).expect("Invalid slot");
    }
    Self {
      kind: Arc::new(RwLock::new(ExprKind::from_api(api.kind, ctx))),
      is_canonical: Arc::default(),
      pos: Pos::from_api(&api.location),
    }
  }
  pub fn to_api(&self) -> api::InspectedKind {
    use api::InspectedKind as K;
    match &*self.kind.read().unwrap() {
      ExprKind::Atom(a) => K::Atom(a.to_api()),
      ExprKind::Bottom(b) => K::Bottom(b.to_api()),
      _ => K::Opaque,
    }
  }
}
impl Drop for Expr {
  fn drop(&mut self) {
    // If the only two references left are this and known, remove from known
    if Arc::strong_count(&self.kind) == 2 && self.is_canonical.load(Ordering::Relaxed) {
      // if known is poisoned, a leak is preferable to a panicking destructor
      if let Ok(mut w) = KNOWN_EXPRS.write() {
        w.remove(&self.id());
      }
    }
  }
}

lazy_static! {
  static ref KNOWN_EXPRS: RwLock<HashMap<api::ExprTicket, Expr>> = RwLock::default();
}

#[derive(Clone, Debug)]
pub enum ExprKind {
  Seq(Expr, Expr),
  Call(Expr, Expr),
  Atom(AtomHand),
  Argument,
  Lambda(Option<PathSet>, Expr),
  Bottom(OrcErrv),
  Const(Sym),
}
impl ExprKind {
  pub fn from_api(api: api::ExpressionKind, ctx: &mut ExprParseCtx) -> Self {
    use api::ExpressionKind as K;
    match api {
      K::Slot(_) => panic!("Handled in Expr"),
      K::Lambda(id, b) => ExprKind::Lambda(PathSet::from_api(id, &b), Expr::from_api(*b, ctx)),
      K::Arg(_) => ExprKind::Argument,
      K::Bottom(b) => ExprKind::Bottom(OrcErrv::from_api(&b)),
      K::Call(f, x) => ExprKind::Call(Expr::from_api(*f, ctx), Expr::from_api(*x, ctx)),
      K::Const(c) => ExprKind::Const(Sym::from_tok(deintern(c)).unwrap()),
      K::NewAtom(a) => ExprKind::Atom(AtomHand::from_api(a)),
      K::Seq(a, b) => ExprKind::Seq(Expr::from_api(*a, ctx), Expr::from_api(*b, ctx)),
    }
  }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Step {
  Left,
  Right,
}

#[derive(Clone, Debug)]
pub struct PathSet {
  /// The single steps through [super::nort::Clause::Apply]
  pub steps: VecDeque<Step>,
  /// if Some, it splits at a [super::nort::Clause::Apply]. If None, it ends in
  /// a [super::nort::Clause::LambdaArg]
  pub next: Option<(Box<PathSet>, Box<PathSet>)>,
}
impl PathSet {
  pub fn after(mut self, step: Step) -> Self {
    self.steps.push_front(step);
    self
  }
  pub fn from_api(id: u64, b: &api::Expression) -> Option<Self> {
    use api::ExpressionKind as K;
    match &b.kind {
      K::Arg(id2) => (id == *id2).then(|| Self { steps: VecDeque::new(), next: None }),
      K::Bottom(_) | K::Const(_) | K::NewAtom(_) | K::Slot(_) => None,
      K::Lambda(_, b) => Self::from_api(id, b),
      K::Call(l, r) | K::Seq(l, r) => match (Self::from_api(id, l), Self::from_api(id, r)) {
        (Some(a), Some(b)) =>
          Some(Self { steps: VecDeque::new(), next: Some((Box::new(a), Box::new(b))) }),
        (Some(l), None) => Some(l.after(Step::Left)),
        (None, Some(r)) => Some(r.after(Step::Right)),
        (None, None) => None,
      },
    }
  }
}
