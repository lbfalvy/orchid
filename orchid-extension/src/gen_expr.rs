use futures::FutureExt;
use orchid_base::error::{OrcErr, OrcErrv};
use orchid_base::location::Pos;
use orchid_base::match_mapping;
use orchid_base::name::Sym;
use orchid_base::reqnot::ReqHandlish;

use crate::api;
use crate::atom::{AtomFactory, ToAtom};
use crate::conv::{ToExpr, TryFromExpr};
use crate::expr::Expr;
use crate::func_atom::Lambda;
use crate::system::SysCtx;

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
	cont: impl Fn(I) -> O + Clone + Send + Sync + 'static,
) -> GExpr {
	call([lambda(0, [seq([arg(0), call([Lambda::new(cont).to_expr(), arg(0)])])]), expr])
}
