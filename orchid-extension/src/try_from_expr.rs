use crate::error::ProjectResult;
use crate::expr::{ExprHandle, OwnedExpr};

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
