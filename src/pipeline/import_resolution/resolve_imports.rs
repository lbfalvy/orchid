use std::rc::Rc;

use super::alias_map::AliasMap;
use super::apply_aliases::apply_aliases;
use super::collect_aliases::collect_aliases;
use super::decls::{InjectedAsFn, UpdatedFn};
use crate::interner::Interner;
use crate::pipeline::error::ProjectError;
use crate::pipeline::project_tree::ProjectTree;

/// Follow import chains to locate the original name of all tokens, then
/// replace these aliases with the original names throughout the tree
pub fn resolve_imports(
  project: ProjectTree,
  i: &Interner,
  injected_as: &impl InjectedAsFn,
  updated: &impl UpdatedFn,
) -> Result<ProjectTree, Rc<dyn ProjectError>> {
  let mut map = AliasMap::new();
  collect_aliases(project.0.as_ref(), &project, &mut map, i, updated)?;
  let new_mod =
    apply_aliases(project.0.as_ref(), &map, i, injected_as, updated);
  Ok(ProjectTree(Rc::new(new_mod)))
}
