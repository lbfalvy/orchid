use std::hash::Hash;

use hashbrown::{HashMap, HashSet};

use crate::{interner::Tok, VName};

#[derive(Clone, Debug, Default)]
pub struct AliasMap {
  pub targets: HashMap<VName, VName>,
  pub aliases: HashMap<VName, HashSet<VName>>,
}
impl AliasMap {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn link(&mut self, alias: VName, target: VName) {
    let prev = self.targets.insert(alias.clone(), target.clone());
    debug_assert!(prev.is_none(), "Alias already has a target");
    multimap_entry(&mut self.aliases, &target).insert(alias.clone());
    // Remove aliases of the alias
    if let Some(alts) = self.aliases.remove(&alias) {
      for alt in alts {
        // Assert that this step has always been done in the past
        debug_assert!(
          self.aliases.get(&alt).map(HashSet::is_empty).unwrap_or(true),
          "Alias set of alias not empty"
        );
        let alt_target = self.targets.insert(alt.clone(), target.clone());
        debug_assert!(
          alt_target.as_ref() == Some(&alias),
          "Name not target of its own alias"
        );
        multimap_entry(&mut self.aliases, &alias).insert(alt);
      }
    }
  }

  pub fn resolve(&self, alias: &[Tok<String>]) -> Option<&VName> {
    self.targets.get(alias)
  }
}

/// find or create the set belonging to the given key in the given
/// map-to-set (aka. multimap)
fn multimap_entry<'a, K: Eq + Hash + Clone, V>(
  map: &'a mut HashMap<K, HashSet<V>>,
  key: &'_ K,
) -> &'a mut HashSet<V> {
  map
    .raw_entry_mut()
    .from_key(key)
    .or_insert_with(|| (key.clone(), HashSet::new()))
    .1
}
