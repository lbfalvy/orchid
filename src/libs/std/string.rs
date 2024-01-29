//! `std::string` String processing

use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;

use intern_all::{i, Tok};
use unicode_segmentation::UnicodeSegmentation;

use super::runtime_error::RuntimeError;
use crate::foreign::atom::Atomic;
use crate::foreign::error::ExternResult;
use crate::foreign::inert::{Inert, InertPayload};
use crate::foreign::to_clause::ToClause;
use crate::foreign::try_from_expr::TryFromExpr;
use crate::gen::tree::{xfn_ent, ConstTree};
use crate::interpreter::nort::{Clause, Expr};
use crate::location::CodeLocation;
use crate::utils::iter_find::iter_find;

/// An Orchid string which may or may not be interned
#[derive(Clone, Eq)]
pub enum OrcString {
  /// An interned string. Equality-conpared by reference.
  Interned(Tok<String>),
  /// An uninterned bare string. Equality-compared by character
  Runtime(Arc<String>),
}

impl Debug for OrcString {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Runtime(s) => write!(f, "r\"{s}\""),
      Self::Interned(t) => write!(f, "i\"{t}\""),
    }
  }
}

impl OrcString {
  /// Intern the contained string
  pub fn intern(&mut self) {
    if let Self::Runtime(t) = self {
      *self = Self::Interned(i(t.as_str()))
    }
  }
  /// Clone out the plain Rust [String]
  #[must_use]
  pub fn get_string(self) -> String {
    match self {
      Self::Interned(s) => s.as_str().to_owned(),
      Self::Runtime(rc) => Arc::try_unwrap(rc).unwrap_or_else(|rc| (*rc).clone()),
    }
  }
}

impl Deref for OrcString {
  type Target = String;

  fn deref(&self) -> &Self::Target {
    match self {
      Self::Interned(t) => t,
      Self::Runtime(r) => r,
    }
  }
}

impl Hash for OrcString {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) { self.as_str().hash(state) }
}

impl From<String> for OrcString {
  fn from(value: String) -> Self { Self::Runtime(Arc::new(value)) }
}

impl From<Tok<String>> for OrcString {
  fn from(value: Tok<String>) -> Self { Self::Interned(value) }
}

impl PartialEq for OrcString {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Interned(t1), Self::Interned(t2)) => t1 == t2,
      _ => **self == **other,
    }
  }
}

impl InertPayload for OrcString {
  const TYPE_STR: &'static str = "OrcString";
  fn strict_eq(&self, other: &Self) -> bool { self == other }
}

impl ToClause for String {
  fn to_clause(self, _: CodeLocation) -> Clause { Inert(OrcString::from(self)).atom_cls() }
}

impl TryFromExpr for String {
  fn from_expr(exi: Expr) -> ExternResult<Self> {
    Ok(exi.downcast::<Inert<OrcString>>()?.0.get_string())
  }
}

pub(super) fn str_lib() -> ConstTree {
  ConstTree::ns("std::string", [ConstTree::tree([
    xfn_ent("slice", [|s: Inert<OrcString>, i: Inert<usize>, len: Inert<usize>| {
      let graphs = s.0.as_str().graphemes(true);
      if i.0 == 0 {
        return Ok(graphs.take(len.0).collect::<String>());
      }
      let mut prefix = graphs.skip(i.0 - 1);
      if prefix.next().is_none() {
        return Err(RuntimeError::ext(
          "Character index out of bounds".to_string(),
          "indexing string",
        ));
      }
      let mut count = 0;
      let ret = (prefix.take(len.0))
        .map(|x| {
          count += 1;
          x
        })
        .collect::<String>();
      if count == len.0 {
        Ok(ret)
      } else {
        RuntimeError::fail("Character index out of bounds".to_string(), "indexing string")
      }
    }]),
    xfn_ent("concat", [|a: String, b: Inert<OrcString>| a + b.0.as_str()]),
    xfn_ent("find", [|haystack: Inert<OrcString>, needle: Inert<OrcString>| {
      let haystack_graphs = haystack.0.as_str().graphemes(true);
      iter_find(haystack_graphs, needle.0.as_str().graphemes(true)).map(Inert)
    }]),
    xfn_ent("split", [|s: String, i: Inert<usize>| -> (String, String) {
      let mut graphs = s.as_str().graphemes(true);
      (graphs.by_ref().take(i.0).collect(), graphs.collect())
    }]),
    xfn_ent("len", [|s: Inert<OrcString>| Inert(s.0.graphemes(true).count())]),
    xfn_ent("size", [|s: Inert<OrcString>| Inert(s.0.as_bytes().len())]),
    xfn_ent("intern", [|s: Inert<OrcString>| {
      Inert(match s.0 {
        OrcString::Runtime(s) => OrcString::Interned(i(&*s)),
        x => x,
      })
    }]),
  ])])
}
