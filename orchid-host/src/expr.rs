use std::collections::VecDeque;
use std::fmt;
use std::num::NonZeroU64;
use std::rc::{Rc, Weak};

use async_std::sync::RwLock;
use futures::FutureExt;
use hashbrown::HashSet;
use itertools::Itertools;
use orchid_api::ExprTicket;
use orchid_base::error::OrcErrv;
use orchid_base::format::{FmtCtx, FmtUnit, Format, Variants};
use orchid_base::location::Pos;
use orchid_base::name::Sym;
use orchid_base::tree::{AtomRepr, indent};
use orchid_base::{match_mapping, tl_cache};

use crate::api;
use crate::atom::AtomHand;
use crate::extension::Extension;

pub type ExprParseCtx = Extension;

#[derive(Debug)]
pub struct ExprData {
	pos: Pos,
	kind: RwLock<ExprKind>,
}

#[derive(Clone, Debug)]
pub struct Expr(Rc<ExprData>);
impl Expr {
	pub fn pos(&self) -> Pos { self.0.pos.clone() }
	pub fn as_atom(&self) -> Option<AtomHand> { todo!() }
	pub fn strong_count(&self) -> usize { todo!() }
	pub fn id(&self) -> api::ExprTicket {
		api::ExprTicket(
			NonZeroU64::new(self.0.as_ref() as *const ExprData as usize as u64)
				.expect("this is a ref, it cannot be null"),
		)
	}
	pub async fn from_api(api: &api::Expression, ctx: &mut ExprParseCtx) -> Self {
		if let api::ExpressionKind::Slot(tk) = &api.kind {
			return ctx.exprs().get_expr(*tk).expect("Invalid slot");
		}
		let pos = Pos::from_api(&api.location, &ctx.ctx().i).await;
		let kind = RwLock::new(ExprKind::from_api(&api.kind, pos.clone(), ctx).boxed_local().await);
		Self(Rc::new(ExprData { pos, kind }))
	}
	pub async fn to_api(&self) -> api::InspectedKind {
		use api::InspectedKind as K;
		match &*self.0.kind.read().await {
			ExprKind::Atom(a) => K::Atom(a.to_api().await),
			ExprKind::Bottom(b) => K::Bottom(b.to_api()),
			_ => K::Opaque,
		}
	}
}
impl Format for Expr {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		return print_expr(self, c, &mut HashSet::new()).await;
		async fn print_expr<'a>(
			expr: &'a Expr,
			c: &'a (impl FmtCtx + ?Sized + 'a),
			visited: &mut HashSet<ExprTicket>,
		) -> FmtUnit {
			if visited.contains(&expr.id()) {
				return "CYCLIC_EXPR".to_string().into();
			}
			visited.insert(expr.id());
			match &*expr.0.kind.read().await {
				ExprKind::Arg => "Arg".to_string().into(),
				ExprKind::Atom(a) => a.print(c).await,
				ExprKind::Bottom(e) if e.len() == 1 => format!("Bottom({e})").into(),
				ExprKind::Bottom(e) => format!("Bottom(\n\t{}\n)", indent(&e.to_string())).into(),
				ExprKind::Call(f, x) => tl_cache!(Rc<Variants>: Rc::new(Variants::default()
					.unbounded("{0} {1l}")
					.bounded("({0} {1b})")))
				.units([
					print_expr(f, c, visited).boxed_local().await,
					print_expr(x, c, visited).boxed_local().await,
				]),
				ExprKind::Const(c) => format!("{c}").into(),
				ExprKind::Lambda(None, body) => tl_cache!(Rc<Variants>: Rc::new(Variants::default()
					.unbounded("\\.{0l}")
					.bounded("(\\.{0b})")))
				.units([print_expr(body, c, visited).boxed_local().await]),
				ExprKind::Lambda(Some(path), body) => tl_cache!(Rc<Variants>: Rc::new(Variants::default()
					.unbounded("\\{0b}. {1l}")
					.bounded("(\\{0b}. {1b})")))
				.units([format!("{path}").into(), print_expr(body, c, visited).boxed_local().await]),
				ExprKind::Seq(l, r) =>
					tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("[{0b}]{1l}"))).units([
						print_expr(l, c, visited).boxed_local().await,
						print_expr(r, c, visited).boxed_local().await,
					]),
			}
		}
	}
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
	pub async fn from_api(api: &api::ExpressionKind, pos: Pos, ctx: &mut ExprParseCtx) -> Self {
		match_mapping!(api, api::ExpressionKind => ExprKind {
			Lambda(id => PathSet::from_api(*id, api), b => Expr::from_api(b, ctx).await),
			Bottom(b => OrcErrv::from_api(b, &ctx.ctx().i).await),
			Call(f => Expr::from_api(f, ctx).await, x => Expr::from_api(x, ctx).await),
			Const(c => Sym::from_api(*c, &ctx.ctx().i).await),
			Seq(a => Expr::from_api(a, ctx).await, b => Expr::from_api(b, ctx).await),
		} {
			api::ExpressionKind::Arg(_) => ExprKind::Arg,
			api::ExpressionKind::NewAtom(a) => ExprKind::Atom(AtomHand::from_api(
				a,
				pos,
				&mut ctx.ctx().clone()
			).await),
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
			K::Call(l, r) | K::Seq(l, r) => {
				match (Self::from_api(id, &l.kind), Self::from_api(id, &r.kind)) {
					(Some(a), Some(b)) =>
						Some(Self { steps: VecDeque::new(), next: Some((Box::new(a), Box::new(b))) }),
					(Some(l), None) => Some(l.after(Step::Left)),
					(None, Some(r)) => Some(r.after(Step::Right)),
					(None, None) => None,
				}
			},
		}
	}
}
impl fmt::Display for PathSet {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fn print_step(step: Step) -> &'static str { if step == Step::Left { "l" } else { "r" } }
		let step_s = self.steps.iter().copied().map(print_step).join("");
		match &self.next {
			Some((left, right)) => {
				if !step_s.is_empty() {
					write!(f, "{step_s}>")?;
				}
				write!(f, "({left}|{right})")
			},
			None => write!(f, "{step_s}"),
		}
	}
}

pub struct WeakExpr(Weak<ExprData>);
impl WeakExpr {
	pub fn upgrade(&self) -> Option<Expr> { self.0.upgrade().map(Expr) }
}
