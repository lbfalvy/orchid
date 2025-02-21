use orchid_api_derive::Coding;
use orchid_base::error::OrcRes;
use orchid_base::format::FmtUnit;
use orchid_base::number::Numeric;
use orchid_extension::atom::{
	AtomFactory, Atomic, AtomicFeatures, MethodSetBuilder, ToAtom, TypAtom,
};
use orchid_extension::atom_thin::{ThinAtom, ThinVariant};
use orchid_extension::conv::TryFromExpr;
use orchid_extension::expr::Expr;
use orchid_extension::system::SysCtx;
use ordered_float::NotNan;
use rust_decimal::prelude::Zero;

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
		Ok(Self(Num::try_from_expr(expr).await?.0.to_f64()))
	}
}

pub struct Num(pub Numeric);
impl TryFromExpr for Num {
	async fn try_from_expr(expr: Expr) -> OrcRes<Self> {
		let e = match Int::try_from_expr(expr.clone()).await {
			Ok(t) => return Ok(Num(Numeric::Int(t.0))),
			Err(e) => e,
		};
		match TypAtom::<Float>::try_from_expr(expr).await {
			Ok(t) => Ok(Num(Numeric::Float(t.0))),
			Err(e2) => Err(e + e2),
		}
	}
}
impl ToAtom for Num {
	fn to_atom_factory(self) -> AtomFactory {
		match self.0 {
			Numeric::Float(f) => Float(f).factory(),
			Numeric::Int(i) => Int(i).factory(),
		}
	}
}

/// A homogenous fixed length number array that forces all of its elements into
/// the weakest element type. This describes the argument casting behaviour of
/// most numeric operations.
pub enum HomoArray<const N: usize> {
	Int([i64; N]),
	Float([NotNan<f64>; N]),
}
impl<const N: usize> HomoArray<N> {
	pub fn new(n: [Numeric; N]) -> Self {
		let mut ints = [0i64; N];
		for i in 0..N {
			if let Numeric::Int(val) = n[i] {
				ints[i] = val
			} else {
				let mut floats = [NotNan::zero(); N];
				for (i, int) in ints.iter().take(i).enumerate() {
					floats[i] = NotNan::new(*int as f64).expect("i64 cannot convert to f64 NaN");
				}
				for j in i..N {
					floats[j] = n[j].to_f64();
				}
				return Self::Float(floats);
			}
		}
		Self::Int(ints)
	}
}
