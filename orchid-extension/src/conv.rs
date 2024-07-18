use orchid_base::location::Pos;

use crate::atom::{AtomicFeatures, TypAtom};
use crate::error::{ProjectError, ProjectResult};
use crate::expr::{atom, bot_obj, ExprHandle, GenExpr, OwnedExpr};
use crate::system::downcast_atom;

pub trait TryFromExpr: Sized {
  fn try_from_expr(expr: ExprHandle) -> ProjectResult<Self>;
}

impl TryFromExpr for OwnedExpr {
  fn try_from_expr(expr: ExprHandle) -> ProjectResult<Self> { Ok(OwnedExpr::new(expr)) }
}

impl<T: TryFromExpr, U: TryFromExpr> TryFromExpr for (T, U) {
  fn try_from_expr(expr: ExprHandle) -> ProjectResult<Self> {
    Ok((T::try_from_expr(expr.clone())?, U::try_from_expr(expr)?))
  }
}

pub struct ErrorNotAtom(Pos);
impl ProjectError for ErrorNotAtom {
  const DESCRIPTION: &'static str = "Expected an atom";
  fn one_position(&self) -> Pos { self.0.clone() }
}

pub struct ErrorUnexpectedType(Pos);
impl ProjectError for ErrorUnexpectedType {
  const DESCRIPTION: &'static str = "Type error";
  fn one_position(&self) -> Pos { self.0.clone() }
}

impl<A: AtomicFeatures> TryFromExpr for TypAtom<A> {
  fn try_from_expr(expr: ExprHandle) -> ProjectResult<Self> {
    OwnedExpr::new(expr)
      .foreign_atom()
      .map_err(|ex| ErrorNotAtom(ex.pos.clone()).pack())
      .and_then(|f| downcast_atom(f).map_err(|f| ErrorUnexpectedType(f.pos).pack()))
  }
}

pub trait ToExpr {
  fn to_expr(self) -> GenExpr;
}

impl<T: ToExpr> ToExpr for ProjectResult<T> {
  fn to_expr(self) -> GenExpr {
    match self {
      Err(e) => bot_obj(e),
      Ok(t) => t.to_expr(),
    }
  }
}

impl<A: AtomicFeatures> ToExpr for A {
  fn to_expr(self) -> GenExpr { atom(self) }
}
