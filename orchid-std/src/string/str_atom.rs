use std::borrow::Cow;
use std::io;
use std::ops::Deref;
use std::sync::Arc;

use orchid_api_derive::Coding;
use orchid_api_traits::{Encode, Request};
use orchid_base::error::{mk_errv, OrcRes};
use orchid_base::intern;
use orchid_base::interner::{intern, Tok};
use orchid_extension::atom::{AtomMethod, Atomic, MethodSet, Supports, TypAtom};
use orchid_extension::atom_owned::{DeserializeCtx, OwnedAtom, OwnedVariant};
use orchid_extension::conv::TryFromExpr;
use orchid_extension::expr::Expr;
use orchid_extension::system::SysCtx;

#[derive(Copy, Clone, Coding)]
pub struct StringGetVal;
impl Request for StringGetVal {
  type Response = Arc<String>;
}
impl AtomMethod for StringGetVal {
  const NAME: &str = "std::string_get_val";
}
impl Supports<StringGetVal> for StrAtom {
  fn handle(&self, _: SysCtx, _: StringGetVal) -> <StringGetVal as Request>::Response {
    self.0.clone()
  }
}

#[derive(Clone)]
pub struct StrAtom(Arc<String>);
impl Atomic for StrAtom {
  type Variant = OwnedVariant;
  type Data = ();
  fn reg_reqs() -> MethodSet<Self> { MethodSet::new().handle::<StringGetVal>() }
}
impl StrAtom {
  pub fn new(str: Arc<String>) -> Self { Self(str) }
  pub fn value(&self) -> Arc<String> { self.0.clone() }
}
impl Deref for StrAtom {
  type Target = str;
  fn deref(&self) -> &Self::Target { &self.0 }
}
impl OwnedAtom for StrAtom {
  type Refs = ();
  fn val(&self) -> Cow<'_, Self::Data> { Cow::Owned(()) }
  fn serialize(&self, _: SysCtx, sink: &mut (impl io::Write + ?Sized)) -> Self::Refs {
    self.deref().encode(sink)
  }
  fn deserialize(mut ctx: impl DeserializeCtx, _: Self::Refs) -> Self {
    Self::new(Arc::new(ctx.read::<String>()))
  }
}

#[derive(Debug, Clone)]
pub struct IntStrAtom(Tok<String>);
impl Atomic for IntStrAtom {
  type Variant = OwnedVariant;
  type Data = orchid_api::TStr;
  fn reg_reqs() -> MethodSet<Self> { MethodSet::new() }
}
impl From<Tok<String>> for IntStrAtom {
  fn from(value: Tok<String>) -> Self { Self(value) }
}
impl OwnedAtom for IntStrAtom {
  type Refs = ();
  fn val(&self) -> Cow<'_, Self::Data> { Cow::Owned(self.0.to_api()) }
  fn print(&self, _ctx: SysCtx) -> String { format!("{:?}i", *self.0) }
  fn serialize(&self, _: SysCtx, write: &mut (impl io::Write + ?Sized)) { self.0.encode(write) }
  fn deserialize(ctx: impl DeserializeCtx, _: ()) -> Self { Self(intern(&ctx.decode::<String>())) }
}

#[derive(Clone)]
pub enum OrcString<'a> {
  Val(TypAtom<'a, StrAtom>),
  Int(TypAtom<'a, IntStrAtom>),
}
impl<'a> OrcString<'a> {
  pub fn get_string(&self) -> Arc<String> {
    match &self {
      Self::Int(tok) => Tok::from_api(tok.value).arc(),
      Self::Val(atom) => atom.request(StringGetVal),
    }
  }
}

impl TryFromExpr for OrcString<'static> {
  fn try_from_expr(expr: Expr) -> OrcRes<OrcString<'static>> {
    if let Ok(v) = TypAtom::<StrAtom>::try_from_expr(expr.clone()) {
      return Ok(OrcString::Val(v));
    }
    match TypAtom::<IntStrAtom>::try_from_expr(expr) {
      Ok(t) => Ok(OrcString::Int(t)),
      Err(e) => Err(mk_errv(intern!(str: "A string was expected"), "", e.pos_iter())),
    }
  }
}
