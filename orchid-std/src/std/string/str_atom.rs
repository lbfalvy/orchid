use std::borrow::Cow;
use std::ops::Deref;
use std::pin::Pin;
use std::rc::Rc;

use async_std::io::Write;
use orchid_api_derive::Coding;
use orchid_api_traits::{Encode, Request};
use orchid_base::error::{OrcRes, mk_errv};
use orchid_base::format::{FmtCtx, FmtUnit};
use orchid_base::interner::Tok;
use orchid_extension::atom::{AtomMethod, Atomic, MethodSetBuilder, Supports, TypAtom};
use orchid_extension::atom_owned::{DeserializeCtx, OwnedAtom, OwnedVariant};
use orchid_extension::conv::TryFromExpr;
use orchid_extension::expr::Expr;
use orchid_extension::system::SysCtx;

#[derive(Copy, Clone, Debug, Coding)]
pub struct StringGetVal;
impl Request for StringGetVal {
	type Response = Rc<String>;
}
impl AtomMethod for StringGetVal {
	const NAME: &str = "std::string_get_val";
}
impl Supports<StringGetVal> for StrAtom {
	async fn handle(&self, _: SysCtx, _: StringGetVal) -> <StringGetVal as Request>::Response {
		self.0.clone()
	}
}

#[derive(Clone)]
pub struct StrAtom(Rc<String>);
impl Atomic for StrAtom {
	type Variant = OwnedVariant;
	type Data = ();
	fn reg_reqs() -> MethodSetBuilder<Self> { MethodSetBuilder::new().handle::<StringGetVal>() }
}
impl StrAtom {
	pub fn new(str: Rc<String>) -> Self { Self(str) }
}
impl Deref for StrAtom {
	type Target = str;
	fn deref(&self) -> &Self::Target { &self.0 }
}
impl OwnedAtom for StrAtom {
	type Refs = ();
	async fn val(&self) -> Cow<'_, Self::Data> { Cow::Owned(()) }
	async fn serialize(&self, _: SysCtx, sink: Pin<&mut (impl Write + ?Sized)>) -> Self::Refs {
		self.deref().encode(sink).await
	}
	async fn print<'a>(&'a self, _: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		format!("{:?}", &*self.0).into()
	}
	async fn deserialize(mut ctx: impl DeserializeCtx, _: Self::Refs) -> Self {
		Self::new(Rc::new(ctx.read::<String>().await))
	}
}

#[derive(Debug, Clone)]
pub struct IntStrAtom(Tok<String>);
impl Atomic for IntStrAtom {
	type Variant = OwnedVariant;
	type Data = orchid_api::TStr;
}
impl From<Tok<String>> for IntStrAtom {
	fn from(value: Tok<String>) -> Self { Self(value) }
}
impl OwnedAtom for IntStrAtom {
	type Refs = ();
	async fn val(&self) -> Cow<'_, Self::Data> { Cow::Owned(self.0.to_api()) }
	async fn print<'a>(&'a self, _: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		format!("{:?}i", *self.0).into()
	}
	async fn serialize(&self, _: SysCtx, write: Pin<&mut (impl Write + ?Sized)>) {
		self.0.encode(write).await
	}
	async fn deserialize(mut ctx: impl DeserializeCtx, _: ()) -> Self {
		let s = ctx.decode::<String>().await;
		Self(ctx.sys().i().i(&s).await)
	}
}

#[derive(Clone)]
pub struct OrcString<'a> {
	kind: OrcStringKind<'a>,
	ctx: SysCtx,
}

#[derive(Clone)]
pub enum OrcStringKind<'a> {
	Val(TypAtom<'a, StrAtom>),
	Int(TypAtom<'a, IntStrAtom>),
}
impl OrcString<'_> {
	pub async fn get_string(&self) -> Rc<String> {
		match &self.kind {
			OrcStringKind::Int(tok) => self.ctx.i().ex(**tok).await.rc(),
			OrcStringKind::Val(atom) => atom.request(StringGetVal).await,
		}
	}
}

impl TryFromExpr for OrcString<'static> {
	async fn try_from_expr(expr: Expr) -> OrcRes<OrcString<'static>> {
		if let Ok(v) = TypAtom::<StrAtom>::try_from_expr(expr.clone()).await {
			return Ok(OrcString { ctx: expr.ctx(), kind: OrcStringKind::Val(v) });
		}
		let ctx = expr.ctx();
		match TypAtom::<IntStrAtom>::try_from_expr(expr).await {
			Ok(t) => Ok(OrcString { ctx: t.data.ctx(), kind: OrcStringKind::Int(t) }),
			Err(e) => Err(mk_errv(ctx.i().i("A string was expected").await, "", e.pos_iter())),
		}
	}
}
