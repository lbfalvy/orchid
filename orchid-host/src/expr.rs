use std::cell::RefCell;
use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::rc::{Rc, Weak};
use std::{fmt, mem};

use async_std::sync::RwLock;
use futures::FutureExt;
use hashbrown::HashSet;
use itertools::Itertools;
use orchid_base::error::{OrcErrv, mk_errv};
use orchid_base::format::{FmtCtx, FmtCtxImpl, FmtUnit, Format, Variants, take_first};
use orchid_base::location::Pos;
use orchid_base::macros::mtreev_fmt;
use orchid_base::name::Sym;
use orchid_base::tokens::Paren;
use orchid_base::tree::{AtomRepr, indent};
use orchid_base::{match_mapping, tl_cache};
use substack::Substack;

use crate::api;
use crate::atom::AtomHand;
use crate::ctx::Ctx;
use crate::extension::Extension;
use crate::macros::{MacTok, MacTree};

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
	pub fn from_api(id: u64, api: &api::ExpressionKind) -> Option<Self> {
		use api::ExpressionKind as K;
		struct Suffix(VecDeque<Step>, Option<(Box<PathSet>, Box<PathSet>)>);
		fn seal(Suffix(steps, next): Suffix) -> PathSet { PathSet { steps: steps.into(), next } }
		fn after(step: Step, mut suf: Suffix) -> Suffix {
			suf.0.push_front(step);
			suf
		}
		return from_api_inner(id, api).map(seal);
		fn from_api_inner(id: u64, api: &api::ExpressionKind) -> Option<Suffix> {
			match &api {
				K::Arg(id2) => (id == *id2).then_some(Suffix(VecDeque::new(), None)),
				K::Bottom(_) | K::Const(_) | K::NewAtom(_) | K::Slot(_) => None,
				K::Lambda(_, b) => from_api_inner(id, &b.kind),
				K::Call(l, r) | K::Seq(l, r) => {
					match (from_api_inner(id, &l.kind), from_api_inner(id, &r.kind)) {
						(Some(a), Some(b)) =>
							Some(Suffix(VecDeque::new(), Some((Box::new(seal(a)), Box::new(seal(b)))))),
						(Some(l), None) => Some(after(Step::Left, l)),
						(None, Some(r)) => Some(after(Step::Right, r)),
						(None, None) => None,
					}
				},
			}
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

#[derive(Clone)]
pub enum SrcToExprStep<'a> {
	Left,
	Right,
	Lambda(Sym, &'a RefCell<Option<PathSet>>),
}

pub async fn mtreev_to_expr(
	src: &[MacTree],
	stack: Substack<'_, SrcToExprStep<'_>>,
	ctx: &Ctx,
) -> ExprKind {
	let Some((x, f)) = src.split_last() else { panic!("Empty expression cannot be evaluated") };
	let x_stack = if f.is_empty() { stack.clone() } else { stack.push(SrcToExprStep::Right) };
	let x_kind = match &*x.tok {
		MacTok::Atom(a) => ExprKind::Atom(a.clone()),
		MacTok::Name(n) => 'name: {
			let mut steps = VecDeque::new();
			for step in x_stack.iter() {
				match step {
					SrcToExprStep::Left => steps.push_front(Step::Left),
					SrcToExprStep::Right => steps.push_front(Step::Right),
					SrcToExprStep::Lambda(name, _) if name != n => continue,
					SrcToExprStep::Lambda(_, cell) => {
						let mut ps = cell.borrow_mut();
						match &mut *ps {
							val @ None => *val = Some(PathSet { steps: steps.into(), next: None }),
							Some(val) => {
								let mut swap = PathSet { steps: Vec::new(), next: None };
								mem::swap(&mut swap, val);
								*val = merge(swap, &Vec::from(steps));
								fn merge(ps: PathSet, steps: &[Step]) -> PathSet {
									let diff_idx = ps.steps.iter().zip(steps).take_while(|(l, r)| l == r).count();
									if diff_idx == ps.steps.len() {
										if diff_idx == steps.len() {
											match ps.next {
												Some(_) => panic!("New path ends where old path forks"),
												None => panic!("New path same as old path"),
											}
										}
										let Some((left, right)) = ps.next else {
											panic!("Old path ends where new path continues")
										};
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
							},
						}
						break 'name ExprKind::Arg;
					},
				}
			}
			ExprKind::Const(n.clone())
		},
		MacTok::Ph(_) | MacTok::Done(_) | MacTok::Ref(_) | MacTok::Slot(_) =>
			ExprKind::Bottom(mk_errv(
				ctx.i.i("placeholder in value").await,
				"Placeholders cannot appear anywhere outside macro patterns",
				[x.pos.clone().into()],
			)),
		MacTok::S(Paren::Round, b) if b.is_empty() =>
			return ExprKind::Bottom(mk_errv(
				ctx.i.i("Empty expression").await,
				"Empty parens () are illegal",
				[x.pos.clone().into()],
			)),
		MacTok::S(Paren::Round, b) => mtreev_to_expr(b, x_stack, ctx).boxed_local().await,
		MacTok::S(..) => ExprKind::Bottom(mk_errv(
			ctx.i.i("non-round parentheses after macros").await,
			"[] or {} block was not consumed by macros; expressions may only contain ()",
			[x.pos.clone().into()],
		)),
		MacTok::Lambda(_, b) if b.is_empty() =>
			return ExprKind::Bottom(mk_errv(
				ctx.i.i("Empty lambda").await,
				"Lambdas must have a body",
				[x.pos.clone().into()],
			)),
		MacTok::Lambda(arg, b) => 'lambda_converter: {
			if let [MacTree { tok, .. }] = &**arg {
				if let MacTok::Name(n) = &**tok {
					let path = RefCell::new(None);
					let b = mtreev_to_expr(b, x_stack.push(SrcToExprStep::Lambda(n.clone(), &path)), ctx)
						.boxed_local()
						.await;
					break 'lambda_converter ExprKind::Lambda(path.into_inner(), b.at(x.pos.clone()));
				}
			}
			let argstr = take_first(&mtreev_fmt(arg, &FmtCtxImpl { i: &ctx.i }).await, true);
			ExprKind::Bottom(mk_errv(
				ctx.i.i("Malformeed lambda").await,
				format!("Lambda argument should be single name, found {argstr}"),
				[x.pos.clone().into()],
			))
		},
	};
	if f.is_empty() {
		return x_kind;
	}
	let f = mtreev_to_expr(f, stack.push(SrcToExprStep::Left), ctx).boxed_local().await;
	ExprKind::Call(f.at(Pos::None), x_kind.at(x.pos.clone()))
}
