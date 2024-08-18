use std::borrow::Cow;
use std::io;
use std::num::NonZeroU64;
use std::sync::Arc;

use never::Never;
use orchid_api_derive::Coding;
use orchid_api_traits::{Encode, Request};
use orchid_base::error::{mk_err, OrcRes};
use orchid_base::id_store::IdStore;
use orchid_base::intern;
use orchid_base::interner::{deintern, intern, Tok};
use orchid_extension::atom::{Atomic, ReqPck, TypAtom};
use orchid_extension::atom_owned::{DeserializeCtx, OwnedAtom, OwnedVariant};
use orchid_extension::conv::TryFromExpr;
use orchid_extension::expr::ExprHandle;
use orchid_extension::system::SysCtx;

pub static STR_REPO: IdStore<Arc<String>> = IdStore::new();

#[derive(Copy, Clone, Coding)]
pub struct StringGetVal;
impl Request for StringGetVal {
  type Response = String;
}

pub struct StrAtom(NonZeroU64);
impl Atomic for StrAtom {
  type Variant = OwnedVariant;
  type Data = NonZeroU64;
  type Req = StringGetVal;
}
impl StrAtom {
  pub fn new(str: Arc<String>) -> Self { Self(STR_REPO.add(str).id()) }
}
impl Clone for StrAtom {
  fn clone(&self) -> Self { Self::new(self.local_value()) }
}
impl StrAtom {
  fn try_local_value(&self) -> Option<Arc<String>> { STR_REPO.get(self.0).map(|r| r.clone()) }
  fn local_value(&self) -> Arc<String> { self.try_local_value().expect("no string found for ID") }
}
impl OwnedAtom for StrAtom {
  type Refs = ();
  fn val(&self) -> Cow<'_, Self::Data> { Cow::Owned(self.0) }
  fn same(&self, _: SysCtx, other: &Self) -> bool { self.local_value() == other.local_value() }
  fn handle_req(&self, pck: impl ReqPck<Self>) { self.local_value().encode(pck.unpack().1) }
  fn serialize(&self, _: SysCtx, sink: &mut (impl io::Write + ?Sized)) -> Self::Refs {
    self.local_value().encode(sink)
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
  type Req = Never;
}
impl From<Tok<String>> for IntStrAtom {
  fn from(value: Tok<String>) -> Self { Self(value) }
}
impl OwnedAtom for IntStrAtom {
  type Refs = ();
  fn val(&self) -> Cow<'_, Self::Data> { Cow::Owned(self.0.marker()) }
  fn handle_req(&self, pck: impl ReqPck<Self>) { pck.never() }
  fn print(&self, _ctx: SysCtx) -> String { format!("{:?}i", self.0.as_str()) }
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
      Self::Int(tok) => deintern(tok.value).arc(),
      Self::Val(atom) => match STR_REPO.get(**atom) {
        Some(rec) => rec.clone(),
        None => Arc::new(atom.request(StringGetVal)),
      },
    }
  }
}

impl TryFromExpr for OrcString<'static> {
  fn try_from_expr(expr: ExprHandle) -> OrcRes<OrcString<'static>> {
    if let Ok(v) = TypAtom::<StrAtom>::downcast(expr.clone()) {
      return Ok(OrcString::Val(v));
    }
    match TypAtom::<IntStrAtom>::downcast(expr) {
      Ok(t) => Ok(OrcString::Int(t)),
      Err(e) => Err(vec![mk_err(intern!(str: "A string was expected"), "", [e.0.clone().into()])]),
    }
  }
}
