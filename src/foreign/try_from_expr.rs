use super::error::ExternResult;
use crate::interpreter::nort::{ClauseInst, Expr};
use crate::location::CodeLocation;

/// Types automatically convertible from an [Expr]. Most notably, this is how
/// foreign functions request automatic argument downcasting.
pub trait TryFromExpr: Sized {
  /// Match and clone the value out of an [Expr]
  fn from_expr(expr: Expr) -> ExternResult<Self>;
}

impl TryFromExpr for Expr {
  fn from_expr(expr: Expr) -> ExternResult<Self> { Ok(expr) }
}

impl TryFromExpr for ClauseInst {
  fn from_expr(expr: Expr) -> ExternResult<Self> { Ok(expr.clsi()) }
}

/// Request a value of a particular type and also return its location for
/// further error reporting
#[derive(Debug, Clone)]
pub struct WithLoc<T>(pub CodeLocation, pub T);
impl<T: TryFromExpr> TryFromExpr for WithLoc<T> {
  fn from_expr(expr: Expr) -> ExternResult<Self> {
    Ok(Self(expr.location(), T::from_expr(expr)?))
  }
}
