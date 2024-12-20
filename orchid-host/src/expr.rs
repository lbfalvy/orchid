use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use hashbrown::HashMap;
use lazy_static::lazy_static;
use orchid_base::error::OrcErrv;
use orchid_base::location::Pos;
use orchid_base::match_mapping;
use orchid_base::name::Sym;
use orchid_base::tree::AtomRepr;

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
  pub fn from_api(api: &api::Expression, ctx: &mut ExprParseCtx) -> Self {
    if let api::ExpressionKind::Slot(tk) = &api.kind {
      return Self::resolve(*tk).expect("Invalid slot");
    }
    Self {
      kind: Arc::new(RwLock::new(ExprKind::from_api(&api.kind, ctx))),
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
  Arg,
  Lambda(Option<PathSet>, Expr),
  Bottom(OrcErrv),
  Const(Sym),
}
impl ExprKind {
  pub fn from_api(api: &api::ExpressionKind, ctx: &mut ExprParseCtx) -> Self {
    match_mapping!(api, api::ExpressionKind => ExprKind {
      Lambda(id => PathSet::from_api(*id, api), b => Expr::from_api(b, ctx)),
      Bottom(b => OrcErrv::from_api(b)),
      Call(f => Expr::from_api(f, ctx), x => Expr::from_api(x, ctx)),
      Const(c => Sym::from_api(*c)),
      Seq(a => Expr::from_api(a, ctx), b => Expr::from_api(b, ctx)),
    } {
      api::ExpressionKind::Arg(_) => ExprKind::Arg,
      api::ExpressionKind::NewAtom(a) => ExprKind::Atom(AtomHand::from_api(a.clone())),
      api::ExpressionKind::Slot(_) => panic!("Handled in Expr"),
    })
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
  pub fn from_api(id: u64, api: &api::ExpressionKind) -> Option<Self> {
    use api::ExpressionKind as K;
    match &api {
      K::Arg(id2) => (id == *id2).then(|| Self { steps: VecDeque::new(), next: None }),
      K::Bottom(_) | K::Const(_) | K::NewAtom(_) | K::Slot(_) => None,
      K::Lambda(_, b) => Self::from_api(id, &b.kind),
      K::Call(l, r) | K::Seq(l, r) =>
        match (Self::from_api(id, &l.kind), Self::from_api(id, &r.kind)) {
          (Some(a), Some(b)) =>
            Some(Self { steps: VecDeque::new(), next: Some((Box::new(a), Box::new(b))) }),
          (Some(l), None) => Some(l.after(Step::Left)),
          (None, Some(r)) => Some(r.after(Step::Right)),
          (None, None) => None,
        },
    }
  }
}
