//! Components to build in-memory module trees that in Orchid. These modules
//! can only contain constants and other modules.

use std::fmt::Debug;

use dyn_clone::{clone_box, DynClone};
use trait_set::trait_set;

use super::tpl;
use super::traits::{Gen, GenClause};
use crate::foreign::atom::Atomic;
use crate::interpreter::gen_nort::nort_gen;
use crate::interpreter::nort::Expr;
use crate::location::CodeLocation;
use crate::tree::{ModEntry, ModMember, TreeConflict};
use crate::utils::combine::Combine;

trait_set! {
  trait TreeLeaf = Gen<Expr, [Expr; 0]> + DynClone;
}

/// A leaf in the [ConstTree]
#[derive(Debug)]
pub struct GenConst(Box<dyn TreeLeaf>);
impl GenConst {
  fn new(data: impl GenClause + Clone + 'static) -> Self {
    Self(Box::new(data))
  }
  /// Instantiate template as [crate::interpreter::nort]
  pub fn gen_nort(&self, location: CodeLocation) -> Expr {
    self.0.template(nort_gen(location), [])
  }
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
  fn combine(self, _: Self) -> Result<Self, Self::Error> {
    Err(ConflictingConsts)
  }
}

/// A lightweight module tree that can be built declaratively by hand to
/// describe libraries of external functions in Rust. It implements [Add] for
/// added convenience
pub type ConstTree = ModEntry<GenConst, (), ()>;

/// Describe a constant
#[must_use]
pub fn leaf(value: impl GenClause + Clone + 'static) -> ConstTree {
  ModEntry { x: (), member: ModMember::Item(GenConst::new(value)) }
}

/// Describe an [Atomic]
#[must_use]
pub fn atom_leaf(atom: impl Atomic + Clone + 'static) -> ConstTree {
  leaf(tpl::V(atom))
}

/// Describe an [Atomic] which appears as an entry in a [ConstTree#tree]
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

/// Errors produced duriung the merger of constant trees
pub type ConstCombineErr = TreeConflict<GenConst, (), ()>;
