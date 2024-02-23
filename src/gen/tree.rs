//! Components to build in-memory module trees that in Orchid. These modules
//! can only contain constants and other modules.

use std::fmt;

use dyn_clone::{clone_box, DynClone};
use intern_all::Tok;
use itertools::Itertools;
use substack::Substack;
use trait_set::trait_set;

use super::tpl;
use super::traits::{Gen, GenClause};
use crate::foreign::atom::{AtomGenerator, Atomic};
use crate::foreign::fn_bridge::{xfn, Xfn};
use crate::interpreter::gen_nort::nort_gen;
use crate::interpreter::nort::Expr;
use crate::location::CodeLocation;
use crate::tree::{ModEntry, ModMember, TreeConflict};
use crate::utils::combine::Combine;

trait_set! {
  trait TreeLeaf = Gen<Expr, [Expr; 0]> + DynClone + Send;
  trait XfnCB = FnOnce(Substack<Tok<String>>) -> AtomGenerator + DynClone + Send;
}

enum GCKind {
  Const(Box<dyn TreeLeaf>),
  Xfn(usize, Box<dyn XfnCB>),
}

/// A leaf in the [ConstTree]
pub struct GenConst(GCKind);
impl GenConst {
  fn c(data: impl GenClause + Send + Clone + 'static) -> Self {
    Self(GCKind::Const(Box::new(data)))
  }
  fn f<const N: usize, Argv, Ret>(f: impl Xfn<N, Argv, Ret>) -> Self {
    Self(GCKind::Xfn(
      N,
      Box::new(move |stck| AtomGenerator::cloner(xfn(&stck.unreverse().iter().join("::"), f))),
    ))
  }
  /// Instantiate as [crate::interpreter::nort]
  pub fn gen_nort(self, stck: Substack<Tok<String>>, location: CodeLocation) -> Expr {
    match self.0 {
      GCKind::Const(c) => c.template(nort_gen(location), []),
      GCKind::Xfn(_, cb) => tpl::AnyAtom(cb(stck)).template(nort_gen(location), []),
    }
  }
}
impl fmt::Debug for GenConst {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match &self.0 {
      GCKind::Const(c) => write!(f, "{c:?}"),
      GCKind::Xfn(n, _) => write!(f, "xfn/{n}"),
    }
  }
}
impl Clone for GenConst {
  fn clone(&self) -> Self {
    match &self.0 {
      GCKind::Const(c) => Self(GCKind::Const(clone_box(&**c))),
      GCKind::Xfn(n, cb) => Self(GCKind::Xfn(*n, clone_box(&**cb))),
    }
  }
}

/// Error condition when constant trees that define the the same constant are
/// merged. Produced during system loading if multiple modules define the
/// same constant
#[derive(Debug, Clone)]
pub struct ConflictingConsts;

impl Combine for GenConst {
  type Error = ConflictingConsts;
  fn combine(self, _: Self) -> Result<Self, Self::Error> { Err(ConflictingConsts) }
}

/// A lightweight module tree that can be built declaratively by hand to
/// describe libraries of external functions in Rust. It implements [Combine]
/// for merging libraries.
pub type ConstTree = ModEntry<GenConst, (), ()>;

/// Describe a constant
#[must_use]
pub fn leaf(value: impl GenClause + Clone + Send + 'static) -> ConstTree {
  ModEntry::wrap(ModMember::Item(GenConst::c(value)))
}

/// Describe a constant which appears in [ConstTree::tree].
///
/// The unarray tricks rustfmt into keeping this call as a single line even if
/// it chooses to break the argument into a block.
pub fn ent<K: AsRef<str>>(
  key: K,
  [g]: [impl GenClause + Clone + Send + 'static; 1],
) -> (K, ConstTree) {
  (key, leaf(g))
}

/// Describe an [Atomic]
#[must_use]
pub fn atom_leaf(atom: impl Atomic + Clone + Send + 'static) -> ConstTree { leaf(tpl::V(atom)) }

/// Describe an [Atomic] which appears as an entry in a [ConstTree::tree]
///
/// The unarray is used to trick rustfmt into breaking the atom into a block
/// without breaking this call into a block
#[must_use]
pub fn atom_ent<K: AsRef<str>>(
  key: K,
  [atom]: [impl Atomic + Clone + 'static; 1],
) -> (K, ConstTree) {
  (key, atom_leaf(atom))
}

/// Describe a function
pub fn xfn_leaf<const N: usize, Argv, Ret>(f: impl Xfn<N, Argv, Ret>) -> ConstTree {
  ModEntry::wrap(ModMember::Item(GenConst::f(f)))
}

/// Describe a function which appears as an entry in a [ConstTree::tree]
///
/// The unarray is used to trick rustfmt into breaking the atom into a block
/// without breaking this call into a block
#[must_use]
pub fn xfn_ent<const N: usize, Argv, Ret, K: AsRef<str>>(
  key: K,
  [f]: [impl Xfn<N, Argv, Ret>; 1],
) -> (K, ConstTree) {
  (key, xfn_leaf(f))
}

/// Errors produced duriung the merger of constant trees
pub type ConstCombineErr = TreeConflict<GenConst, (), ()>;
