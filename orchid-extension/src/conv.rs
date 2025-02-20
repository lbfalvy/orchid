use std::future::Future;

use orchid_base::error::{OrcErr, OrcRes, mk_err};
use orchid_base::interner::Interner;
use orchid_base::location::Pos;

use crate::atom::{AtomicFeatures, ToAtom, TypAtom};
use crate::expr::Expr;
use crate::gen_expr::{GExpr, atom, bot};
use crate::system::downcast_atom;

pub trait TryFromExpr: Sized {
	fn try_from_expr(expr: Expr) -> impl Future<Output = OrcRes<Self>>;
}

impl TryFromExpr for Expr {
	async fn try_from_expr(expr: Expr) -> OrcRes<Self> { Ok(expr) }
}

impl<T: TryFromExpr, U: TryFromExpr> TryFromExpr for (T, U) {
	async fn try_from_expr(expr: Expr) -> OrcRes<Self> {
		Ok((T::try_from_expr(expr.clone()).await?, U::try_from_expr(expr).await?))
	}
}

async fn err_not_atom(pos: Pos, i: &Interner) -> OrcErr {
	mk_err(i.i("Expected an atom").await, "This expression is not an atom", [pos.into()])
}

async fn err_type(pos: Pos, i: &Interner) -> OrcErr {
	mk_err(i.i("Type error").await, "The atom is a different type than expected", [pos.into()])
}

impl<A: AtomicFeatures> TryFromExpr for TypAtom<'_, A> {
	async fn try_from_expr(expr: Expr) -> OrcRes<Self> {
		match expr.atom().await {
			Err(ex) => Err(err_not_atom(ex.data().await.pos.clone(), &ex.ctx().i).await.into()),
			Ok(f) => match downcast_atom::<A>(f).await {
				Ok(a) => Ok(a),
				Err(f) => Err(err_type(f.pos(), &f.ctx().i).await.into()),
			},
		}
	}
}

pub trait ToExpr {
	fn to_expr(self) -> GExpr;
}

impl ToExpr for GExpr {
	fn to_expr(self) -> GExpr { self }
}
impl ToExpr for Expr {
	fn to_expr(self) -> GExpr { self.gen() }
}

impl<T: ToExpr> ToExpr for OrcRes<T> {
	fn to_expr(self) -> GExpr {
		match self {
			Err(e) => bot(e),
			Ok(t) => t.to_expr(),
		}
	}
}

impl<A: ToAtom> ToExpr for A {
	fn to_expr(self) -> GExpr { atom(self) }
}
