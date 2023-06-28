use std::rc::Rc;

use super::alias_map::AliasMap;
use super::apply_aliases::apply_aliases;
use super::collect_aliases::collect_aliases;
use super::decls::{InjectedAsFn, UpdatedFn};
use crate::interner::Interner;
use crate::pipeline::error::ProjectError;
use crate::representations::project::ProjectTree;
use crate::representations::VName;

/// Follow import chains to locate the original name of all tokens, then
/// replace these aliases with the original names throughout the tree
pub fn resolve_imports(
  project: ProjectTree<VName>,
  i: &Interner,
  injected_as: &impl InjectedAsFn,
  updated: &impl UpdatedFn,
) -> Result<ProjectTree<VName>, Rc<dyn ProjectError>> {
  let mut map = AliasMap::new();
  collect_aliases(&project.0, &project, &mut map, i, updated)?;
  let new_mod = apply_aliases(&project.0, &map, injected_as, updated);
  Ok(ProjectTree(new_mod))
}
