//! Components to build in-memory module trees that in Orchid. These modules
//! can only contain constants and other modules.

use std::fmt;

use dyn_clone::{clone_box, DynClone};
use crate::api;
use trait_set::trait_set;

use super::tpl;
use super::traits::{Gen, GenClause};
use crate::combine::Combine;
use crate::tree::{ModEntry, ModMember, TreeConflict};

trait_set! {
  trait TreeLeaf = Gen<Expr, [Expr; 0]> + DynClone + Send;
}

/// A leaf in the [ConstTree]
pub struct GenConst(Box<dyn TreeLeaf>);
impl GenConst {
  fn c(data: impl GenClause + Send + Clone + 'static) -> Self { Self(Box::new(data)) }
}
impl fmt::Debug for GenConst {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{:?}", self.0) }
}
impl Clone for GenConst {
  fn clone(&self) -> Self { Self(clone_box(&*self.0)) }
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
pub fn atom_leaf(atom: Atom) -> ConstTree { leaf(tpl::SysAtom(atom)) }

/// Describe an [Atomic] which appears as an entry in a [ConstTree::tree]
///
/// The unarray is used to trick rustfmt into breaking the atom into a block
/// without breaking this call into a block
#[must_use]
pub fn atom_ent<K: AsRef<str>>(key: K, [atom]: [Atom; 1]) -> (K, ConstTree) {
  (key, atom_leaf(atom))
}

/// Errors produced duriung the merger of constant trees
pub type ConstCombineErr = TreeConflict<GenConst, (), ()>;
