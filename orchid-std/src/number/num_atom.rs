use never::Never;
use orchid_api_derive::Coding;
use orchid_extension::atom::{Atomic, ReqPck, TypAtom};
use orchid_extension::atom_thin::{ThinAtom, ThinVariant};
use orchid_extension::conv::{ToExpr, TryFromExpr};
use orchid_extension::error::{pack_err, ProjectResult};
use orchid_extension::expr::{ExprHandle, GenExpr};
use orchid_extension::system::SysCtx;
use ordered_float::NotNan;

#[derive(Clone, Debug, Coding)]
pub struct Int(pub i64);
impl Atomic for Int {
  type Variant = ThinVariant;
  type Data = Self;
  type Req = Never;
}
impl ThinAtom for Int {
  fn handle_req(&self, _ctx: SysCtx, pck: impl ReqPck<Self>) { pck.never() }
}
impl TryFromExpr for Int {
  fn try_from_expr(expr: ExprHandle) -> ProjectResult<Self> {
    TypAtom::<Int>::try_from_expr(expr).map(|t| t.value)
  }
}

#[derive(Clone, Debug, Coding)]
pub struct Float(pub NotNan<f64>);
impl Atomic for Float {
  type Variant = ThinVariant;
  type Data = Self;
  type Req = Never;
}
impl ThinAtom for Float {
  fn handle_req(&self, _ctx: SysCtx, pck: impl ReqPck<Self>) { pck.never() }
}
impl TryFromExpr for Float {
  fn try_from_expr(expr: ExprHandle) -> ProjectResult<Self> {
    TypAtom::<Float>::try_from_expr(expr).map(|t| t.value)
  }
}

pub enum Numeric {
  Int(i64),
  Float(NotNan<f64>),
}
impl TryFromExpr for Numeric {
  fn try_from_expr(expr: ExprHandle) -> ProjectResult<Self> {
    Int::try_from_expr(expr.clone()).map(|t| Numeric::Int(t.0)).or_else(|e| {
      Float::try_from_expr(expr).map(|t| Numeric::Float(t.0)).map_err(|e2| pack_err([e, e2]))
    })
  }
}
impl ToExpr for Numeric {
  fn to_expr(self) -> GenExpr {
    match self {
      Self::Float(f) => Float(f).to_expr(),
      Self::Int(i) => Int(i).to_expr(),
    }
  }
}
