use hashbrown::{HashMap, HashSet};

use std::hash::Hash;

use crate::interner::Token;

#[derive(Clone, Debug, Default)]
pub struct AliasMap{
  pub targets: HashMap<Token<Vec<Token<String>>>, Token<Vec<Token<String>>>>,
  pub aliases: HashMap<Token<Vec<Token<String>>>, HashSet<Token<Vec<Token<String>>>>>,
}
impl AliasMap {
  pub fn new() -> Self {Self::default()}

  pub fn link(&mut self, alias: Token<Vec<Token<String>>>, target: Token<Vec<Token<String>>>) {
    let prev = self.targets.insert(alias, target);
    debug_assert!(prev.is_none(), "Alias already has a target");
    multimap_entry(&mut self.aliases, &target).insert(alias);
    // Remove aliases of the alias
    if let Some(alts) = self.aliases.remove(&alias) {
      for alt in alts {
        // Assert that this step has always been done in the past
        debug_assert!(
          self.aliases.get(&alt)
            .map(HashSet::is_empty)
            .unwrap_or(true),
          "Alias set of alias not empty"
        );
        debug_assert!(
          self.targets.insert(alt, target) == Some(alias),
          "Name not target of its own alias"
        );
        multimap_entry(&mut self.aliases, &target).insert(alt);
      }
    }
  }

  pub fn resolve(&self, alias: Token<Vec<Token<String>>>) -> Option<Token<Vec<Token<String>>>> {
    self.targets.get(&alias).copied()
  }
}

/// find or create the set belonging to the given key in the given
/// map-to-set (aka. multimap)
fn multimap_entry<'a, K: Eq + Hash + Clone, V>(
  map: &'a mut HashMap<K, HashSet<V>>,
  key: &'_ K
) -> &'a mut HashSet<V> {
  map.raw_entry_mut()
    .from_key(key)
    .or_insert_with(|| (key.clone(), HashSet::new()))
    .1
}