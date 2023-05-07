use std::ops::Add;
use std::rc::Rc;
use hashbrown::HashMap;

use crate::interner::Token;
use crate::utils::Substack;

use super::sourcefile::Import;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModMember<TItem: Clone, TExt: Clone>{
  Item(TItem),
  Sub(Rc<Module<TItem, TExt>>)
}
impl<TItem: Clone, TExt: Clone> ModMember<TItem, TExt> {
  #[allow(unused)]
  pub fn item(&self) -> &TItem {
    if let Self::Item(it) = self {it} else {
      panic!("Expected item, found submodule")
    }
  }

  #[allow(unused)]
  pub fn sub(&self) -> &Rc<Module<TItem, TExt>> {
    if let Self::Sub(sub) = self {sub} else {
      panic!("Expected submodule, found item")
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModEntry<TItem: Clone, TExt: Clone>{
  pub member: ModMember<TItem, TExt>,
  pub exported: bool
}

/// A module, containing imports, 
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module<TItem: Clone, TExt: Clone>{
  pub imports: Vec<Import>,
  pub items: HashMap<Token<String>, ModEntry<TItem, TExt>>,
  pub extra: TExt
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WalkErrorKind {
  Private,
  Missing
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WalkError {
  pub pos: usize,
  pub kind: WalkErrorKind
}

pub type ModPath<'a> = Substack<'a, Token<String>>;
impl<TItem: Clone, TExt: Clone> Module<TItem, TExt> {
  pub fn walk(self: &Rc<Self>,
    path: &[Token<String>], require_exported: bool
  ) -> Result<Rc<Self>, WalkError> {
    let mut cur = self;
    for (pos, step) in path.iter().enumerate() {
      if let Some(ModEntry{
        member: ModMember::Sub(next),
        exported,
      }) = cur.items.get(step) {
        if require_exported && !exported {
          return Err(WalkError{ pos, kind: WalkErrorKind::Private })
        }
        cur = next
      } else {
        return Err(WalkError{ pos, kind: WalkErrorKind::Missing })
      }
    }
    Ok(cur.clone())
  }

  fn visit_all_imports_rec<E>(&self,
    path: ModPath,
    callback: &mut impl FnMut(ModPath, &Self, &Import) -> Result<(), E>
  ) -> Result<(), E> {
    for import in self.imports.iter() {
      callback(path, self, import)?
    }
    for (name, entry) in self.items.iter() {
      if let ModMember::Sub(module) = &entry.member {
        module.visit_all_imports_rec(
          path.push(*name),
          callback
        )?
      }
    }
    Ok(())
  }

  pub fn visit_all_imports<E>(&self,
    callback: &mut impl FnMut(ModPath, &Self, &Import) -> Result<(), E>
  ) -> Result<(), E> {
    self.visit_all_imports_rec(Substack::Bottom, callback)
  }
}

impl<TItem: Clone, TExt: Clone> Add for Module<TItem, TExt>
where TExt: Add<Output = TExt>
{
  type Output = Self;

  fn add(mut self, rhs: Self) -> Self::Output {
    let Module{ extra, imports, items } = rhs;
    for (key, right) in items {
      // if both contain a submodule
      if let Some(left) = self.items.remove(&key) {
        if let ModMember::Sub(rsub) = &right.member {
          if let ModMember::Sub(lsub) = &left.member {
            // merge them with rhs exportedness
            let new_mod = lsub.as_ref().clone() + rsub.as_ref().clone();
            self.items.insert(key, ModEntry{
              exported: right.exported,
              member: ModMember::Sub(Rc::new(new_mod))
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