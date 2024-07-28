use std::borrow::Cow;
use std::num::NonZeroU64;
use std::sync::Arc;

use never::Never;
use orchid_api::interner::TStr;
use orchid_api_derive::Coding;
use orchid_api_traits::{Encode, Request};
use orchid_base::id_store::IdStore;
use orchid_base::interner::{deintern, Tok};
use orchid_base::location::Pos;
use orchid_extension::atom::{Atomic, ReqPck, TypAtom};
use orchid_extension::atom_owned::{OwnedAtom, OwnedVariant};
use orchid_extension::conv::TryFromExpr;
use orchid_extension::error::{ProjectError, ProjectResult};
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
  fn clone(&self) -> Self { Self(STR_REPO.add(STR_REPO.get(self.0).unwrap().clone()).id()) }
}
impl StrAtom {
  fn try_local_value(&self) -> Option<Arc<String>> { STR_REPO.get(self.0).map(|r| r.clone()) }
  fn local_value(&self) -> Arc<String> { self.try_local_value().expect("no string found for ID") }
}
impl OwnedAtom for StrAtom {
  fn val(&self) -> Cow<'_, Self::Data> { Cow::Owned(self.0) }
  fn same(&self, _ctx: SysCtx, other: &Self) -> bool { self.local_value() == other.local_value() }
  fn handle_req(&self, _ctx: SysCtx, pck: impl ReqPck<Self>) {
    self.local_value().encode(pck.unpack().1)
  }
}

#[derive(Debug, Clone)]
pub struct IntStrAtom(Tok<String>);
impl Atomic for IntStrAtom {
  type Variant = OwnedVariant;
  type Data = TStr;
  type Req = Never;
}
impl From<Tok<String>> for IntStrAtom {
  fn from(value: Tok<String>) -> Self { Self(value) }
}
impl OwnedAtom for IntStrAtom {
  fn val(&self) -> Cow<'_, Self::Data> { Cow::Owned(self.0.marker()) }
  fn handle_req(&self, _ctx: SysCtx, pck: impl ReqPck<Self>) { pck.never() }
}

#[derive(Clone)]
pub enum OrcString {
  Val(TypAtom<StrAtom>),
  Int(Tok<String>),
}
impl OrcString {
  pub fn get_string(&self) -> Arc<String> {
    match &self {
      Self::Int(tok) => tok.arc(),
      Self::Val(atom) => match STR_REPO.get(**atom) {
        Some(rec) => rec.clone(),
        None => Arc::new(atom.request(StringGetVal)),
      },
    }
  }
}
impl From<Tok<String>> for OrcString {
  fn from(value: Tok<String>) -> Self { OrcString::Int(value) }
}

pub struct NotString(Pos);
impl ProjectError for NotString {
  const DESCRIPTION: &'static str = "A string was expected";
  fn one_position(&self) -> Pos { self.0.clone() }
}
impl TryFromExpr for OrcString {
  fn try_from_expr(expr: ExprHandle) -> ProjectResult<OrcString> {
    if let Ok(v) = TypAtom::<StrAtom>::downcast(expr.clone()) {
      return Ok(OrcString::Val(v));
    }
    match TypAtom::<IntStrAtom>::downcast(expr) {
      Ok(t) => Ok(OrcString::Int(deintern(*t))),
      Err(e) => Err(NotString(e.0).pack()),
    }
  }
}
