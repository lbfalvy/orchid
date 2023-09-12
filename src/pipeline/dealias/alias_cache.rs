use std::slice;

use chumsky::primitive::Container;
use hashbrown::HashMap;

use crate::representations::project::{ProjectMod, ItemKind, ProjectEntry};
use crate::tree::ModMember;
use crate::utils::{pushed, unwrap_or};
use crate::{ProjectTree, VName, Tok, NameLike};

use super::walk_with_links::{walk_with_links, Target};

pub struct AliasCache {
  data: HashMap<Vec<Tok<String>>, Option<Vec<Tok<String>>>>,
}
impl AliasCache {
  pub fn new() -> Self {
    Self { data: HashMap::new() }
  }

  /// Finds the absolute nsname corresponding to the given name in the given
  /// context, if it's imported. If the name is defined locally, returns None
  /// to avoid allocating several vectors for every local variable.
  pub fn resolv_name<'a>(
    &'a mut self,
    root: &ProjectMod<VName>,
    location: &[Tok<String>],
    name: Tok<String>
  ) -> Option<&'a [Tok<String>]> {
    let full_path = pushed(location, name);
    if let Some(result) = self.data.get(&full_path) {
      return result.as_deref();
    }
    let (ent, finalp) = walk_with_links(root, location.iter().cloned())
      .expect("This path should be valid");
    let m = unwrap_or!{ent => Target::Mod; panic!("Must be a module")};
    let result = m.extra.imports_from.get(&name).map(|next| {
      self.resolv_name(root, &next, name).unwrap_or(&next)
    });
    self.data.insert(full_path, result.map(|s| s.to_vec()));
    return result;
  }

  /// Find the absolute target of a 
  pub fn resolv_vec<'a>(
    &'a mut self,
    root: &ProjectMod<VName>,
    modname: &[Tok<String>],
    vname: &[Tok<String>],
  ) -> Option<&'a [Tok<String>]> {
    let (name, ns) = vname.split_last().expect("name cannot be empty");
    if ns.is_empty() {
      self.resolv_name(modname, name)
    } else {
      let origin = self.resolv_vec(modname, ns)?;
      self.resolv_name(origin, name)
    }
  }
}
