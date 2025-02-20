use orchid_api_derive::Coding;
use orchid_base::error::OrcRes;
use orchid_base::format::FmtUnit;
use orchid_extension::atom::{
	AtomFactory, Atomic, AtomicFeatures, MethodSetBuilder, ToAtom, TypAtom,
};
use orchid_extension::atom_thin::{ThinAtom, ThinVariant};
use orchid_extension::conv::TryFromExpr;
use orchid_extension::expr::Expr;
use orchid_extension::system::SysCtx;
use ordered_float::NotNan;

#[derive(Clone, Debug, Coding)]
pub struct Int(pub i64);
impl Atomic for Int {
	type Variant = ThinVariant;
	type Data = Self;
	fn reg_reqs() -> MethodSetBuilder<Self> { MethodSetBuilder::new() }
}
impl ThinAtom for Int {
	async fn print(&self, _: SysCtx) -> FmtUnit { self.0.to_string().into() }
}
impl TryFromExpr for Int {
	async fn try_from_expr(expr: Expr) -> OrcRes<Self> {
		TypAtom::<Int>::try_from_expr(expr).await.map(|t| t.value)
	}
}

#[derive(Clone, Debug, Coding)]
pub struct Float(pub NotNan<f64>);
impl Atomic for Float {
	type Variant = ThinVariant;
	type Data = Self;
	fn reg_reqs() -> MethodSetBuilder<Self> { MethodSetBuilder::new() }
}
impl ThinAtom for Float {
	async fn print(&self, _: SysCtx) -> FmtUnit { self.0.to_string().into() }
}
impl TryFromExpr for Float {
	async fn try_from_expr(expr: Expr) -> OrcRes<Self> {
		TypAtom::<Float>::try_from_expr(expr).await.map(|t| t.value)
	}
}

pub enum Numeric {
	Int(i64),
	Float(NotNan<f64>),
}
impl TryFromExpr for Numeric {
	async fn try_from_expr(expr: Expr) -> OrcRes<Self> {
		match Int::try_from_expr(expr.clone()).await {
			Ok(t) => Ok(Numeric::Int(t.0)),
			Err(e) => Float::try_from_expr(expr).await.map(|t| Numeric::Float(t.0)).map_err(|e2| e + e2),
		}
	}
}
impl ToAtom for Numeric {
	fn to_atom_factory(self) -> AtomFactory {
		match self {
			Self::Float(f) => Float(f).factory(),
			Self::Int(i) => Int(i).factory(),
		}
	}
}
