use std::borrow::Cow;
use std::num::NonZeroU64;
use std::sync::Arc;

use orchid_api::interner::TStr;
use orchid_api_derive::Coding;
use orchid_api_traits::{Encode, Request};
use orchid_base::id_store::IdStore;
use orchid_base::interner::{deintern, Tok};
use orchid_base::location::Pos;
use orchid_extension::atom::{AtomCard, OwnedAtom, TypAtom};
use orchid_extension::error::{ProjectError, ProjectResult};
use orchid_extension::expr::{ExprHandle, OwnedExpr};
use orchid_extension::system::{downcast_atom, SysCtx};
use orchid_extension::try_from_expr::TryFromExpr;

pub static STR_REPO: IdStore<Arc<String>> = IdStore::new();

#[derive(Clone, Coding)]
pub(crate) enum StringVal {
  Val(NonZeroU64),
  Int(TStr),
}
#[derive(Copy, Clone, Coding)]
pub(crate) struct StringGetVal;
impl Request for StringGetVal {
  type Response = String;
}

pub(crate) enum StringAtom {
  Val(NonZeroU64),
  Int(Tok<String>),
}
impl AtomCard for StringAtom {
  type Data = StringVal;
  type Req = StringGetVal;
}
impl StringAtom {
  pub(crate) fn new_int(tok: Tok<String>) -> Self { Self::Int(tok) }
  pub(crate) fn new(str: Arc<String>) -> Self { Self::Val(STR_REPO.add(str).id()) }
}
impl Clone for StringAtom {
  fn clone(&self) -> Self {
    match &self {
      Self::Int(t) => Self::Int(t.clone()),
      Self::Val(v) => Self::Val(STR_REPO.add(STR_REPO.get(*v).unwrap().clone()).id()),
    }
  }
}
impl StringAtom {
  fn try_local_value(&self) -> Option<Arc<String>> {
    match self {
      Self::Int(tok) => Some(tok.arc()),
      Self::Val(id) => STR_REPO.get(*id).map(|r| r.clone()),
    }
  }
  fn get_value(&self) -> Arc<String> { self.try_local_value().expect("no string found for ID") }
}
impl OwnedAtom for StringAtom {
  fn val(&self) -> Cow<'_, Self::Data> { Cow::Owned(match self {
    Self::Int(tok) => StringVal::Int(tok.marker()),
    Self::Val(id) => StringVal::Val(*id),
  }) }
  fn same(&self, _ctx: SysCtx, other: &Self) -> bool { self.get_value() == other.get_value() }
  fn handle_req(
    &self,
    _ctx: SysCtx,
    StringGetVal: Self::Req,
    rep: &mut (impl std::io::Write + ?Sized),
  ) {
    self.get_value().encode(rep)
  }
}

pub struct OrcString(TypAtom<StringAtom>);
impl OrcString {
  pub fn get_string(&self) -> Arc<String> {
    match &self.0.value {
      StringVal::Int(tok) => deintern(*tok).arc(),
      StringVal::Val(id) => match STR_REPO.get(*id) {
        Some(rec) => rec.clone(),
        None => Arc::new(self.0.request(StringGetVal)),
      },
    }
  }
}
pub struct NotString(Pos);
impl ProjectError for NotString {
  const DESCRIPTION: &'static str = "A string was expected";
  fn one_position(&self) -> Pos {
      self.0.clone()
  }
}
impl TryFromExpr for OrcString {
  fn try_from_expr(expr: ExprHandle) -> ProjectResult<Self> {
    (OwnedExpr::new(expr).foreign_atom().map_err(|expr| expr.position.clone()))
      .and_then(|fatom| downcast_atom(fatom).map_err(|f| f.position))
      .map_err(|p| NotString(p).pack())
      .map(OrcString)
  }
}