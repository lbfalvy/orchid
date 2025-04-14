use std::rc::Rc;

use futures::FutureExt;
use orchid_base::error::{OrcErr, OrcErrv};
use orchid_base::format::{FmtCtx, FmtUnit, Format, Variants};
use orchid_base::location::Pos;
use orchid_base::name::Sym;
use orchid_base::reqnot::ReqHandlish;
use orchid_base::{match_mapping, tl_cache};

use crate::api;
use crate::atom::{AtomFactory, ToAtom};
use crate::conv::{ToExpr, TryFromExpr};
use crate::expr::Expr;
use crate::func_atom::Lambda;
use crate::system::SysCtx;

#[derive(Clone, Debug)]
pub struct GExpr {
	pub kind: GExprKind,
	pub pos: Pos,
}
impl GExpr {
	pub async fn api_return(self, ctx: SysCtx, hand: &impl ReqHandlish) -> api::Expression {
		if let GExprKind::Slot(ex) = self.kind {
			hand.defer_drop(ex.handle());
			api::Expression {
				location: api::Location::SlotTarget,
				kind: api::ExpressionKind::Slot(ex.handle().tk),
			}
		} else {
			api::Expression {
				location: api::Location::Inherit,
				kind: self.kind.api_return(ctx, hand).boxed_local().await,
			}
		}
	}
}
impl Format for GExpr {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		self.kind.print(c).await
	}
}

#[derive(Clone, Debug)]
pub enum GExprKind {
	Call(Box<GExpr>, Box<GExpr>),
	Lambda(u64, Box<GExpr>),
	Arg(u64),
	Seq(Box<GExpr>, Box<GExpr>),
	Const(Sym),
	NewAtom(AtomFactory),
	Slot(Expr),
	Bottom(OrcErrv),
}
impl GExprKind {
	pub async fn api_return(self, ctx: SysCtx, hand: &impl ReqHandlish) -> api::ExpressionKind {
		match_mapping!(self, Self => api::ExpressionKind {
			Call(
				f => Box::new(f.api_return(ctx.clone(), hand).await),
				x => Box::new(x.api_return(ctx, hand).await)
			),
			Seq(
				a => Box::new(a.api_return(ctx.clone(), hand).await),
				b => Box::new(b.api_return(ctx, hand).await)
			),
			Lambda(arg, body => Box::new(body.api_return(ctx, hand).await)),
			Arg(arg),
			Const(name.to_api()),
			Const(name.to_api()),
			Bottom(err.to_api()),
			NewAtom(fac.clone().build(ctx).await),
		} {
			Self::Slot(_) => panic!("processed elsewhere")
		})
	}
}
impl Format for GExprKind {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		match self {
			GExprKind::Call(f, x) =>
				tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("{0} ({1})")))
					.units([f.print(c).await, x.print(c).await]),
			GExprKind::Lambda(arg, body) =>
				tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("\\{0}.{1}")))
					.units([arg.to_string().into(), body.print(c).await]),
			GExprKind::Arg(arg) => arg.to_string().into(),
			GExprKind::Seq(a, b) =>
				tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("[{0}] {1}")))
					.units([a.print(c).await, b.print(c).await]),
			GExprKind::Const(sym) => sym.to_string().into(),
			GExprKind::NewAtom(atom_factory) => atom_factory.to_string().into(),
			GExprKind::Slot(expr) =>
				tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("{{{0}}}")))
					.units([expr.print(c).await]),
			GExprKind::Bottom(orc_errv) => orc_errv.to_string().into(),
		}
	}
}

fn inherit(kind: GExprKind) -> GExpr { GExpr { pos: Pos::Inherit, kind } }

pub fn sym_ref(path: Sym) -> GExpr { inherit(GExprKind::Const(path)) }
pub fn atom<A: ToAtom>(atom: A) -> GExpr { inherit(GExprKind::NewAtom(atom.to_atom_factory())) }

pub fn seq(ops: impl IntoIterator<Item = GExpr>) -> GExpr {
	fn recur(mut ops: impl Iterator<Item = GExpr>) -> Option<GExpr> {
		let op = ops.next()?;
		Some(match recur(ops) {
			None => op,
			Some(rec) => inherit(GExprKind::Seq(Box::new(op), Box::new(rec))),
		})
	}
	recur(ops.into_iter()).expect("Empty list provided to seq!")
}

pub fn arg(n: u64) -> GExpr { inherit(GExprKind::Arg(n)) }

pub fn lambda(n: u64, b: impl IntoIterator<Item = GExpr>) -> GExpr {
	inherit(GExprKind::Lambda(n, Box::new(call(b))))
}

pub fn call(v: impl IntoIterator<Item = GExpr>) -> GExpr {
	v.into_iter()
		.reduce(|f, x| inherit(GExprKind::Call(Box::new(f), Box::new(x))))
		.expect("Empty call expression")
}

pub fn bot(ev: impl IntoIterator<Item = OrcErr>) -> GExpr {
	inherit(GExprKind::Bottom(OrcErrv::new(ev).unwrap()))
}

pub fn with<I: TryFromExpr, O: ToExpr>(
	expr: GExpr,
	cont: impl AsyncFn(I) -> O + Clone + Send + Sync + 'static,
) -> GExpr {
	call([lambda(0, [seq([arg(0), call([Lambda::new(cont).to_expr(), arg(0)])])]), expr])
}
