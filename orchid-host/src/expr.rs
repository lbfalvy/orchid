use std::cell::RefCell;
use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::rc::{Rc, Weak};
use std::{fmt, mem};

use async_std::sync::RwLock;
use futures::FutureExt;
use hashbrown::HashSet;
use itertools::Itertools;
use orchid_base::error::OrcErrv;
use orchid_base::format::{FmtCtx, FmtUnit, Format, Variants};
use orchid_base::location::Pos;
use orchid_base::name::Sym;
use orchid_base::tl_cache;
use orchid_base::tree::{AtomRepr, indent};
use substack::Substack;

use crate::api;
use crate::atom::AtomHand;
use crate::ctx::Ctx;
use crate::expr_store::ExprStore;

#[derive(Clone)]
pub struct ExprParseCtx {
	pub ctx: Ctx,
	pub exprs: ExprStore,
}

#[derive(Debug)]
pub struct ExprData {
	pos: Pos,
	kind: RwLock<ExprKind>,
}

#[derive(Clone, Debug)]
pub struct Expr(Rc<ExprData>);
impl Expr {
	pub fn pos(&self) -> Pos { self.0.pos.clone() }
	pub async fn try_into_owned_atom(self) -> Result<AtomHand, Self> {
		match Rc::try_unwrap(self.0) {
			Err(e) => Err(Self(e)),
			Ok(data) => match data.kind.into_inner() {
				ExprKind::Atom(a) => Ok(a),
				inner => Err(Self(Rc::new(ExprData { kind: inner.into(), pos: data.pos }))),
			},
		}
	}
	pub async fn as_atom(&self) -> Option<AtomHand> {
		if let ExprKind::Atom(a) = &*self.kind().read().await { Some(a.clone()) } else { None }
	}
	pub fn strong_count(&self) -> usize { Rc::strong_count(&self.0) }
	pub fn id(&self) -> api::ExprTicket {
		api::ExprTicket(
			NonZeroU64::new(self.0.as_ref() as *const ExprData as usize as u64)
				.expect("this is a ref, it cannot be null"),
		)
	}
	pub async fn from_api(
		api: &api::Expression,
		psb: PathSetBuilder<'_, u64>,
		ctx: &mut ExprParseCtx,
	) -> Self {
		let pos = Pos::from_api(&api.location, &ctx.ctx.i).await;
		let kind = match &api.kind {
			api::ExpressionKind::Arg(n) => {
				assert!(psb.register_arg(&n), "Arguments must be enclosed in a matching lambda");
				ExprKind::Arg
			},
			api::ExpressionKind::Bottom(bot) =>
				ExprKind::Bottom(OrcErrv::from_api(bot, &ctx.ctx.i).await),
			api::ExpressionKind::Call(f, x) => {
				let (lpsb, rpsb) = psb.split();
				ExprKind::Call(
					Expr::from_api(&f, lpsb, ctx).boxed_local().await,
					Expr::from_api(&x, rpsb, ctx).boxed_local().await,
				)
			},
			api::ExpressionKind::Const(name) => ExprKind::Const(Sym::from_api(*name, &ctx.ctx.i).await),
			api::ExpressionKind::Lambda(x, body) => {
				let lbuilder = psb.lambda(&x);
				let body = Expr::from_api(&body, lbuilder.stack(), ctx).boxed_local().await;
				ExprKind::Lambda(lbuilder.collect(), body)
			},
			api::ExpressionKind::NewAtom(a) =>
				ExprKind::Atom(AtomHand::from_api(a, pos.clone(), &mut ctx.ctx.clone()).await),
			api::ExpressionKind::Slot(tk) => return ctx.exprs.get_expr(*tk).expect("Invalid slot"),
			api::ExpressionKind::Seq(a, b) => {
				let (apsb, bpsb) = psb.split();
				ExprKind::Seq(
					Expr::from_api(&a, apsb, ctx).boxed_local().await,
					Expr::from_api(&b, bpsb, ctx).boxed_local().await,
				)
			},
		};
		Self(Rc::new(ExprData { pos, kind: RwLock::new(kind) }))
	}
	pub async fn to_api(&self) -> api::InspectedKind {
		use api::InspectedKind as K;
		match &*self.0.kind.read().await {
			ExprKind::Atom(a) => K::Atom(a.to_api().await),
			ExprKind::Bottom(b) => K::Bottom(b.to_api()),
			ExprKind::Identity(ex) => ex.to_api().boxed_local().await,
			_ => K::Opaque,
		}
	}
	pub fn kind(&self) -> &RwLock<ExprKind> { &self.0.kind }
}
impl Format for Expr {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		return print_expr(self, c, &mut HashSet::new()).await;
	}
}
async fn print_expr<'a>(
	expr: &'a Expr,
	c: &'a (impl FmtCtx + ?Sized + 'a),
	visited: &mut HashSet<api::ExprTicket>,
) -> FmtUnit {
	if visited.contains(&expr.id()) {
		return "CYCLIC_EXPR".to_string().into();
	}
	visited.insert(expr.id());
	print_exprkind(&*expr.kind().read().await, c, visited).boxed_local().await
}

#[derive(Clone, Debug)]
pub enum ExprKind {
	Seq(Expr, Expr),
	Call(Expr, Expr),
	Atom(AtomHand),
	Arg,
	Lambda(Option<PathSet>, Expr),
	Bottom(OrcErrv),
	Identity(Expr),
	Const(Sym),
	/// Temporary expr kind assigned to a write guard to gain ownership of the
	/// current value during normalization. While this is in place, the guard must
	/// not be dropped.
	Missing,
}
impl ExprKind {
	pub fn at(self, pos: Pos) -> Expr { Expr(Rc::new(ExprData { pos, kind: RwLock::new(self) })) }
}
impl Format for ExprKind {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		print_exprkind(self, c, &mut HashSet::new()).await
	}
}
async fn print_exprkind<'a>(
	ek: &ExprKind,
	c: &'a (impl FmtCtx + ?Sized + 'a),
	visited: &mut HashSet<api::ExprTicket>,
) -> FmtUnit {
	match &ek {
		ExprKind::Arg => "Arg".to_string().into(),
		ExprKind::Missing =>
			panic!("This variant is swapped into write guards, so a read can never see it"),
		ExprKind::Atom(a) => a.print(c).await,
		ExprKind::Bottom(e) if e.len() == 1 => format!("Bottom({e})").into(),
		ExprKind::Bottom(e) => format!("Bottom(\n\t{}\n)", indent(&e.to_string())).into(),
		ExprKind::Call(f, x) => tl_cache!(Rc<Variants>: Rc::new(Variants::default()
			.unbounded("{0} {1l}")
			.bounded("({0} {1b})")))
		.units([print_expr(f, c, visited).await, print_expr(x, c, visited).await]),
		ExprKind::Identity(id) =>
			tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("{{{0}}}"))).units([print_expr(
				id, c, visited,
			)
			.boxed_local()
			.await]),
		ExprKind::Const(c) => format!("{c}").into(),
		ExprKind::Lambda(None, body) => tl_cache!(Rc<Variants>: Rc::new(Variants::default()
			.unbounded("\\.{0l}")
			.bounded("(\\.{0b})")))
		.units([print_expr(body, c, visited).await]),
		ExprKind::Lambda(Some(path), body) => tl_cache!(Rc<Variants>: Rc::new(Variants::default()
			.unbounded("\\{0b}. {1l}")
			.bounded("(\\{0b}. {1b})")))
		.units([format!("{path}").into(), print_expr(body, c, visited).await]),
		ExprKind::Seq(l, r) =>
			tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("[{0b}]{1l}")))
				.units([print_expr(l, c, visited).await, print_expr(r, c, visited).await]),
	}
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Step {
	Left,
	Right,
}

#[derive(Clone)]
pub enum PathSetFrame<'a, T: PartialEq> {
	Lambda(&'a T, &'a RefCell<Option<PathSet>>),
	Step(Step),
}

#[derive(Clone)]
pub struct PathSetBuilder<'a, T: PartialEq>(Substack<'a, PathSetFrame<'a, T>>);
impl<'a, T: PartialEq> PathSetBuilder<'a, T> {
	pub fn new() -> Self { Self(Substack::Bottom) }
	pub fn split(&'a self) -> (Self, Self) {
		(
			Self(self.0.push(PathSetFrame::Step(Step::Left))),
			Self(self.0.push(PathSetFrame::Step(Step::Right))),
		)
	}
	pub fn lambda<'b>(self, arg: &'b T) -> LambdaBuilder<'b, T>
	where 'a: 'b {
		LambdaBuilder { arg, path: RefCell::default(), stack: self }
	}
	/// Register an argument with the corresponding lambda and return true if one
	/// was found. (if false is returned, the name is unbound and may refer to a
	/// global)
	pub fn register_arg(self, t: &T) -> bool {
		let mut steps = VecDeque::new();
		for step in self.0.iter() {
			match step {
				PathSetFrame::Step(step) => steps.push_front(*step),
				PathSetFrame::Lambda(name, _) if **name != *t => (),
				PathSetFrame::Lambda(_, cell) => {
					let mut ps_opt = cell.borrow_mut();
					match &mut *ps_opt {
						val @ None => *val = Some(PathSet { steps: steps.into(), next: None }),
						Some(val) => {
							let mut swap = PathSet { steps: Vec::new(), next: None };
							mem::swap(&mut swap, val);
							*val = merge(swap, &Vec::from(steps));
						},
					}
					return true;
				},
			};
		}
		return false;
		fn merge(ps: PathSet, steps: &[Step]) -> PathSet {
			let diff_idx = ps.steps.iter().zip(steps).take_while(|(l, r)| l == r).count();
			if diff_idx == ps.steps.len() {
				if diff_idx == steps.len() {
					match ps.next {
						Some(_) => panic!("New path ends where old path forks"),
						None => panic!("New path same as old path"),
					}
				}
				let Some((left, right)) = ps.next else { panic!("Old path ends where new path continues") };
				let next = match steps[diff_idx] {
					Step::Left => Some((Box::new(merge(*left, &steps[diff_idx + 1..])), right)),
					Step::Right => Some((left, Box::new(merge(*right, &steps[diff_idx + 1..])))),
				};
				PathSet { steps: ps.steps, next }
			} else {
				let shared_steps = ps.steps.iter().take(diff_idx).cloned().collect();
				let main_steps = ps.steps.iter().skip(diff_idx + 1).cloned().collect();
				let new_branch = steps[diff_idx + 1..].to_vec();
				let main_side = PathSet { steps: main_steps, next: ps.next };
				let new_side = PathSet { steps: new_branch, next: None };
				let (left, right) = match steps[diff_idx] {
					Step::Left => (new_side, main_side),
					Step::Right => (main_side, new_side),
				};
				PathSet { steps: shared_steps, next: Some((Box::new(left), Box::new(right))) }
			}
		}
	}
}

pub struct LambdaBuilder<'a, T: PartialEq> {
	arg: &'a T,
	path: RefCell<Option<PathSet>>,
	stack: PathSetBuilder<'a, T>,
}
impl<'a, T: PartialEq> LambdaBuilder<'a, T> {
	pub fn stack(&'a self) -> PathSetBuilder<'a, T> {
		PathSetBuilder(self.stack.0.push(PathSetFrame::Lambda(self.arg, &self.path)))
	}
	pub fn collect(self) -> Option<PathSet> { self.path.into_inner() }
}

#[derive(Clone, Debug)]
pub struct PathSet {
	/// The single steps through [super::nort::Clause::Apply]
	pub steps: Vec<Step>,
	/// if Some, it splits at a [super::nort::Clause::Apply]. If None, it ends in
	/// a [super::nort::Clause::LambdaArg]
	pub next: Option<(Box<PathSet>, Box<PathSet>)>,
}
impl PathSet {
	pub fn next(&self) -> Option<(&PathSet, &PathSet)> {
		self.next.as_ref().map(|(l, r)| (&**l, &**r))
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
			None => write!(f, "{step_s}x"),
		}
	}
}

pub fn bot_expr(err: impl Into<OrcErrv>) -> Expr {
	let errv: OrcErrv = err.into();
	let pos = errv.pos_iter().next().map_or(Pos::None, |ep| ep.position.clone());
	ExprKind::Bottom(errv).at(pos)
}

pub struct WeakExpr(Weak<ExprData>);
impl WeakExpr {
	pub fn upgrade(&self) -> Option<Expr> { self.0.upgrade().map(Expr) }
}
