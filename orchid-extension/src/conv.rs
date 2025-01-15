use orchid_base::error::{OrcErr, OrcRes, mk_err};
use orchid_base::intern;
use orchid_base::location::Pos;

use crate::atom::{AtomicFeatures, ToAtom, TypAtom};
use crate::expr::{Expr, atom, bot};
use crate::system::downcast_atom;

pub trait TryFromExpr: Sized {
	fn try_from_expr(expr: Expr) -> OrcRes<Self>;
}

impl TryFromExpr for Expr {
	fn try_from_expr(expr: Expr) -> OrcRes<Self> { Ok(expr) }
}

impl<T: TryFromExpr, U: TryFromExpr> TryFromExpr for (T, U) {
	fn try_from_expr(expr: Expr) -> OrcRes<Self> {
		Ok((T::try_from_expr(expr.clone())?, U::try_from_expr(expr)?))
	}
}

async fn err_not_atom(pos: Pos) -> OrcErr {
	mk_err(intern!(str: "Expected an atom").await, "This expression is not an atom", [pos.into()])
}

async fn err_type(pos: Pos) -> OrcErr {
	mk_err(intern!(str: "Type error").await, "The atom is a different type than expected", [
		pos.into()
	])
}

impl<A: AtomicFeatures> TryFromExpr for TypAtom<'_, A> {
	fn try_from_expr(expr: Expr) -> OrcRes<Self> {
		(expr.foreign_atom())
			.map_err(|ex| err_not_atom(ex.pos.clone()).into())
			.and_then(|f| downcast_atom(f).map_err(|f| err_type(f.pos).into()))
	}
}

pub trait ToExpr {
	fn to_expr(self) -> Expr;
}

impl ToExpr for Expr {
	fn to_expr(self) -> Expr { self }
}

impl<T: ToExpr> ToExpr for OrcRes<T> {
	fn to_expr(self) -> Expr {
		match self {
			Err(e) => bot(e),
			Ok(t) => t.to_expr(),
		}
	}
}

impl<A: ToAtom> ToExpr for A {
	fn to_expr(self) -> Expr { atom(self) }
}
