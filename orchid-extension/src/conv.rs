use orchid_base::error::{mk_err, OrcErr, OrcRes};
use orchid_base::intern;
use orchid_base::location::Pos;

use crate::atom::{AtomicFeatures, ToAtom, TypAtom};
use crate::expr::{atom, botv, ExprHandle, GenExpr, OwnedExpr};
use crate::system::downcast_atom;

pub trait TryFromExpr: Sized {
  fn try_from_expr(expr: ExprHandle) -> OrcRes<Self>;
}

impl TryFromExpr for OwnedExpr {
  fn try_from_expr(expr: ExprHandle) -> OrcRes<Self> { Ok(OwnedExpr::new(expr)) }
}

impl<T: TryFromExpr, U: TryFromExpr> TryFromExpr for (T, U) {
  fn try_from_expr(expr: ExprHandle) -> OrcRes<Self> {
    Ok((T::try_from_expr(expr.clone())?, U::try_from_expr(expr)?))
  }
}

fn err_not_atom(pos: Pos) -> OrcErr {
  mk_err(intern!(str: "Expected an atom"), "This expression is not an atom", [pos.into()])
}

fn err_type(pos: Pos) -> OrcErr {
  mk_err(intern!(str: "Type error"), "The atom is a different type than expected", [pos.into()])
}

impl<'a, A: AtomicFeatures> TryFromExpr for TypAtom<'a, A> {
  fn try_from_expr(expr: ExprHandle) -> OrcRes<Self> {
    OwnedExpr::new(expr)
      .foreign_atom()
      .map_err(|ex| vec![err_not_atom(ex.pos.clone())])
      .and_then(|f| downcast_atom(f).map_err(|f| vec![err_type(f.pos)]))
  }
}

pub trait ToExpr {
  fn to_expr(self) -> GenExpr;
}

impl ToExpr for GenExpr {
  fn to_expr(self) -> GenExpr { self }
}

impl<T: ToExpr> ToExpr for OrcRes<T> {
  fn to_expr(self) -> GenExpr {
    match self {
      Err(e) => botv(e),
      Ok(t) => t.to_expr(),
    }
  }
}

impl<A: ToAtom> ToExpr for A {
  fn to_expr(self) -> GenExpr { atom(self) }
}
