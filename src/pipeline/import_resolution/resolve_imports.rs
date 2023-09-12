use super::alias_cache::AliasCache;
use super::alias_map::AliasMap;
use super::apply_aliases::apply_aliases;
use super::collect_aliases::collect_aliases;
use super::decls::{InjectedAsFn, UpdatedFn};
use crate::error::ProjectResult;
use crate::representations::project::ProjectTree;
use crate::representations::VName;

/// Follow import chains to locate the original name of all tokens, then
/// replace these aliases with the original names throughout the tree
pub fn resolve_imports(
  mut project: ProjectTree<VName>,
  injected_as: &impl InjectedAsFn,
  updated: &impl UpdatedFn,
) -> ProjectResult<ProjectTree<VName>> {
  let mut cache = AliasCache::new(&project);
  // let mut map = AliasMap::new();
  // collect_aliases(&project.0, &project, &mut map, updated)?;
  // apply_aliases(&mut project.0, &map, injected_as, updated);
  Ok(project)
}
