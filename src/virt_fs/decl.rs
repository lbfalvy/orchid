use std::rc::Rc;

use intern_all::Tok;

use super::common::CodeNotFound;
use super::{FSResult, Loaded, VirtFS};
use crate::error::ErrorSansLocation;
use crate::name::PathSlice;
use crate::tree::{ModEntry, ModMember};
use crate::utils::combine::Combine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictingTrees;

impl Combine for Rc<dyn VirtFS> {
  type Error = ConflictingTrees;
  fn combine(self, _: Self) -> Result<Self, Self::Error> {
    Err(ConflictingTrees)
  }
}

/// A declarative in-memory tree with [VirtFS] objects for leaves. Paths are
/// followed to a leaf and the leftover handled by it.
pub type DeclTree = ModEntry<Rc<dyn VirtFS>, (), ()>;

impl VirtFS for DeclTree {
  fn get(&self, path: &[Tok<String>], full_path: PathSlice) -> FSResult {
    match &self.member {
      ModMember::Item(it) => it.get(path, full_path),
      ModMember::Sub(module) => match path.split_first() {
        None => Ok(Loaded::collection(module.keys(|_| true))),
        Some((head, tail)) => (module.entries.get(head))
          .ok_or_else(|| CodeNotFound(full_path.to_vpath()).pack())
          .and_then(|ent| ent.get(tail, full_path)),
      },
    }
  }

  fn display(&self, path: &[Tok<String>]) -> Option<String> {
    let (head, tail) = path.split_first()?;
    match &self.member {
      ModMember::Item(it) => it.display(path),
      ModMember::Sub(module) => module.entries.get(head)?.display(tail),
    }
  }
}
