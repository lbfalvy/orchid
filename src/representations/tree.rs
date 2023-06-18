//! Generic module tree structure
//!
//! Used by various stages of the pipeline with different parameters
use std::ops::Add;

use duplicate::duplicate_item;
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
  Sub(Module<TItem, TExt>),
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
  /// Return the module at the end of the given path
  #[allow(clippy::needless_arbitrary_self_type)] // duplicate
  #[duplicate_item(
    method      reference(type) dereference(expr) map_method;
    [walk]      [type]          [expr]            [remove];
    [walk_ref]  [&type]         [*expr]           [get];
    [walk_mut]  [&mut type]     [*expr]           [get_mut];
  )]
  pub fn method(
    self: reference([Self]),
    path: &[Tok<String>],
    require_exported: bool,
  ) -> Result<reference([Self]), WalkError> {
    let mut cur = self;
    for (pos, step) in path.iter().enumerate() {
      if let Some(ModEntry { member: ModMember::Sub(next), exported }) =
        cur.items.map_method(step)
      {
        if require_exported && !dereference([exported]) {
          return Err(WalkError { pos, kind: WalkErrorKind::Private });
        }
        cur = next
      } else {
        return Err(WalkError { pos, kind: WalkErrorKind::Missing });
      }
    }
    Ok(cur)
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

  /// Combine two module trees; wherever they conflict, the overlay is
  /// preferred.
  pub fn overlay(mut self, overlay: Self) -> Self
  where
    TExt: Add<TExt, Output = TExt>,
  {
    let Module { extra, imports, items } = overlay;
    let mut new_items = HashMap::new();
    for (key, right) in items {
      // if both contain a submodule
      match (self.items.remove(&key), right) {
        (
          Some(ModEntry { member: ModMember::Sub(lsub), .. }),
          ModEntry { member: ModMember::Sub(rsub), exported },
        ) => new_items.insert(key, ModEntry {
          exported,
          member: ModMember::Sub(lsub.overlay(rsub)),
        }),
        (_, right) => new_items.insert(key, right),
      };
    }
    new_items.extend(self.items.into_iter());
    self.imports.extend(imports.into_iter());
    Module {
      items: new_items,
      imports: self.imports,
      extra: self.extra + extra,
    }
  }
}
