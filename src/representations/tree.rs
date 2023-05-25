use std::ops::Add;
use std::rc::Rc;

use hashbrown::HashMap;

use super::sourcefile::Import;
use crate::interner::Tok;
use crate::utils::Substack;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModMember<TItem: Clone, TExt: Clone> {
  Item(TItem),
  Sub(Rc<Module<TItem, TExt>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModEntry<TItem: Clone, TExt: Clone> {
  pub member: ModMember<TItem, TExt>,
  pub exported: bool,
}

/// A module, containing imports,
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module<TItem: Clone, TExt: Clone> {
  pub imports: Vec<Import>,
  pub items: HashMap<Tok<String>, ModEntry<TItem, TExt>>,
  pub extra: TExt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WalkErrorKind {
  Private,
  Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WalkError {
  pub pos: usize,
  pub kind: WalkErrorKind,
}

pub type ModPath<'a> = Substack<'a, Tok<String>>;
impl<TItem: Clone, TExt: Clone> Module<TItem, TExt> {
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
