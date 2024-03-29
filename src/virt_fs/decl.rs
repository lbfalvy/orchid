use std::rc::Rc;
use std::sync::Arc;

use intern_all::Tok;

use super::common::CodeNotFound;
use super::{FSResult, Loaded, VirtFS};
use crate::error::ErrorSansOrigin;
use crate::name::PathSlice;
use crate::tree::{ModEntry, ModMember};
use crate::utils::combine::Combine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictingTrees;

impl Combine for Rc<dyn VirtFS> {
  type Error = ConflictingTrees;
  fn combine(self, _: Self) -> Result<Self, Self::Error> { Err(ConflictingTrees) }
}

impl Combine for Arc<dyn VirtFS> {
  type Error = ConflictingTrees;
  fn combine(self, _: Self) -> Result<Self, Self::Error> { Err(ConflictingTrees) }
}

impl<'a> Combine for &'a dyn VirtFS {
  type Error = ConflictingTrees;
  fn combine(self, _: Self) -> Result<Self, Self::Error> { Err(ConflictingTrees) }
}

/// A declarative in-memory tree with [VirtFS] objects for leaves. Paths are
/// followed to a leaf and the leftover handled by it.
pub type DeclTree = ModEntry<Rc<dyn VirtFS>, (), ()>;

impl VirtFS for DeclTree {
  fn get(&self, path: &[Tok<String>], full_path: &PathSlice) -> FSResult {
    match &self.member {
      ModMember::Item(it) => it.get(path, full_path),
      ModMember::Sub(module) => match path.split_first() {
        None => Ok(Loaded::collection(module.keys(|_| true))),
        Some((head, tail)) => (module.entries.get(head))
          .ok_or_else(|| CodeNotFound::new(full_path.to_vpath()).pack())
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

impl VirtFS for String {
  fn display(&self, _: &[Tok<String>]) -> Option<String> { None }
  fn get(&self, path: &[Tok<String>], full_path: &PathSlice) -> FSResult {
    (path.is_empty().then(|| Loaded::Code(Arc::new(self.as_str().to_string()))))
      .ok_or_else(|| CodeNotFound::new(full_path.to_vpath()).pack())
  }
}

impl<'a> VirtFS for &'a str {
  fn display(&self, _: &[Tok<String>]) -> Option<String> { None }
  fn get(&self, path: &[Tok<String>], full_path: &PathSlice) -> FSResult {
    (path.is_empty().then(|| Loaded::Code(Arc::new(self.to_string()))))
      .ok_or_else(|| CodeNotFound::new(full_path.to_vpath()).pack())
  }
}

/// Insert a file by cleartext contents in the [DeclTree].
pub fn decl_file(s: &str) -> DeclTree { DeclTree::leaf(Rc::new(s.to_string())) }
