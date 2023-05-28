//! Generic module tree structure
//!
//! Used by various stages of the pipeline with different parameters
use std::ops::Add;
use std::rc::Rc;

use hashbrown::HashMap;

use super::sourcefile::Import;
use crate::interner::Tok;
use crate::utils::Substack;

/// The member in a [ModEntry] which is associated with a name in a [Module]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModMember<TItem: Clone, TExt: Clone> {
  /// Arbitrary data
  Item(TItem),
  /// A child module
  Sub(Rc<Module<TItem, TExt>>),
}

/// Data about a name in a [Module]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModEntry<TItem: Clone, TExt: Clone> {
  /// The submodule or item
  pub member: ModMember<TItem, TExt>,
  /// Whether the member is visible to modules other than the parent
  pub exported: bool,
}

/// A module, containing imports,
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module<TItem: Clone, TExt: Clone> {
  /// Import statements present this module
  pub imports: Vec<Import>,
  /// Submodules and items by name
  pub items: HashMap<Tok<String>, ModEntry<TItem, TExt>>,
  /// Additional information associated with the module
  pub extra: TExt,
}

/// Possible causes why the path could not be walked
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WalkErrorKind {
  /// `require_exported` was set to `true` and a module wasn't exported
  Private,
  /// A module was not found
  Missing,
}

/// Error produced by [Module::walk]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WalkError {
  /// The 0-based index of the offending segment
  pub pos: usize,
  /// The cause of the error
  pub kind: WalkErrorKind,
}

/// The path taken to reach a given module
pub type ModPath<'a> = Substack<'a, Tok<String>>;

impl<TItem: Clone, TExt: Clone> Module<TItem, TExt> {
  /// Return the module at the end of the given path.
  pub fn walk(
    self: &Rc<Self>,
    path: &[Tok<String>],
    require_exported: bool,
  ) -> Result<Rc<Self>, WalkError> {
    let mut cur = self;
    for (pos, step) in path.iter().enumerate() {
      if let Some(ModEntry { member: ModMember::Sub(next), exported }) =
        cur.items.get(step)
      {
        if require_exported && !exported {
          return Err(WalkError { pos, kind: WalkErrorKind::Private });
        }
        cur = next
      } else {
        return Err(WalkError { pos, kind: WalkErrorKind::Missing });
      }
    }
    Ok(cur.clone())
  }

  fn visit_all_imports_rec<E>(
    &self,
    path: ModPath,
    callback: &mut impl FnMut(ModPath, &Self, &Import) -> Result<(), E>,
  ) -> Result<(), E> {
    for import in self.imports.iter() {
      callback(path, self, import)?
    }
    for (name, entry) in self.items.iter() {
      if let ModMember::Sub(module) = &entry.member {
        module.visit_all_imports_rec(path.push(*name), callback)?
      }
    }
    Ok(())
  }

  /// Call the provided function on every import in the tree. Can be
  /// short-circuited by returning Err
  pub fn visit_all_imports<E>(
    &self,
    callback: &mut impl FnMut(ModPath, &Self, &Import) -> Result<(), E>,
  ) -> Result<(), E> {
    self.visit_all_imports_rec(Substack::Bottom, callback)
  }
}

impl<TItem: Clone, TExt: Clone + Add<Output = TExt>> Add
  for Module<TItem, TExt>
{
  type Output = Self;

  fn add(mut self, rhs: Self) -> Self::Output {
    let Module { extra, imports, items } = rhs;
    for (key, right) in items {
      // if both contain a submodule
      if let Some(left) = self.items.remove(&key) {
        if let ModMember::Sub(rsub) = &right.member {
          if let ModMember::Sub(lsub) = &left.member {
            // merge them with rhs exportedness
            let new_mod = lsub.as_ref().clone() + rsub.as_ref().clone();
            self.items.insert(key, ModEntry {
              exported: right.exported,
              member: ModMember::Sub(Rc::new(new_mod)),
            });
            continue;
          }
        }
      }
      // otherwise right shadows left
      self.items.insert(key, right);
    }
    self.imports.extend(imports.into_iter());
    self.extra = self.extra + extra;
    self
  }
}
