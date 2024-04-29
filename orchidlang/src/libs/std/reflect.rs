//! `std::reflect` Abstraction-breaking operations for dynamically constructing
//! [Clause::Constant] references.

use std::hash::Hash;
use std::sync::atomic::{self, AtomicUsize};
use std::{cmp, fmt};

use intern_all::i;

use super::runtime_error::RuntimeError;
use super::string::OrcString;
use crate::foreign::inert::{Inert, InertPayload};
use crate::foreign::try_from_expr::WithLoc;
use crate::gen::tree::{xfn_ent, ConstTree};
use crate::interpreter::nort::{self, Clause};
use crate::name::Sym;

impl InertPayload for Sym {
  const TYPE_STR: &'static str = "SymbolName";
  fn strict_eq(&self, o: &Self) -> bool { self == o }
}

/// Generate a constant reference at runtime. Referencing a nonexistent constant
/// is a runtime error.
pub fn refer_seq(name: impl IntoIterator<Item = &'static str>) -> Clause {
  Clause::Constant(Sym::new(name.into_iter().map(i)).expect("Empty name"))
}

/// Generate a constant reference at runtime. Referencing a nonexistent constant
/// is a runtime error.
pub fn refer(name: &'static str) -> Clause { refer_seq(name.split("::")) }

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A struct that equals its own copies and only its own copies
#[derive(Clone)]
pub struct RefEqual(usize);
impl RefEqual {
  /// Create a new [RefEqual] which is initially completely unique
  #[allow(clippy::new_without_default)] // new has semantic meaning
  pub fn new() -> Self { Self(COUNTER.fetch_add(1, atomic::Ordering::Relaxed)) }
  /// Return the unique identifier of this [RefEqual] and its copies
  pub fn id(&self) -> usize { self.0 }
}
impl fmt::Debug for RefEqual {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_tuple("RefEqual").field(&self.id()).finish()
  }
}
impl InertPayload for RefEqual {
  const TYPE_STR: &'static str = "RefEqual";
  fn strict_eq(&self, other: &Self) -> bool { self == other }
}
impl Eq for RefEqual {}
impl PartialEq for RefEqual {
  fn eq(&self, other: &Self) -> bool { self.id() == other.id() }
}
impl Ord for RefEqual {
  fn cmp(&self, other: &Self) -> cmp::Ordering { self.id().cmp(&other.id()) }
}
impl PartialOrd for RefEqual {
  fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> { Some(self.cmp(other)) }
}
impl Hash for RefEqual {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) { self.id().hash(state) }
}

pub(super) fn reflect_lib() -> ConstTree {
  ConstTree::ns("std::reflect", [ConstTree::tree([
    xfn_ent("ref_equal", [|l: Inert<RefEqual>, r: Inert<RefEqual>| Inert(l.0.id() == r.0.id())]),
    xfn_ent("modname", [|WithLoc(loc, _): WithLoc<nort::Expr>| Inert(loc.module)]),
    xfn_ent("symbol", [|s: Inert<OrcString>| {
      Sym::parse(s.0.as_str())
        .map(Inert)
        .map_err(|_| RuntimeError::ext("empty string".to_string(), "converting string to Symbol"))
    }]),
  ])])
}
