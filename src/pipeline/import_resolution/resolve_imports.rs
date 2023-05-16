use std::rc::Rc;

use itertools::Itertools;

use crate::interner::Interner;
use crate::pipeline::error::ProjectError;
use crate::pipeline::project_tree::ProjectTree;


use super::alias_map::AliasMap;
use super::collect_aliases::collect_aliases;
use super::apply_aliases::apply_aliases;
use super::decls::InjectedAsFn;

/// Follow import chains to locate the original name of all tokens, then
/// replace these aliases with the original names throughout the tree
pub fn resolve_imports(
  project: ProjectTree,
  i: &Interner,
  injected_as: &impl InjectedAsFn,
) -> Result<ProjectTree, Rc<dyn ProjectError>> {
  let mut map = AliasMap::new();
  collect_aliases(
    project.0.as_ref(),
    &project, &mut map,
    i, injected_as
  )?;
  println!("Aliases: {{{:?}}}",
    map.targets.iter()
      .map(|(kt, vt)| format!("{} => {}",
        i.extern_vec(*kt).join("::"),
        i.extern_vec(*vt).join("::")
      ))
      .join(", ")
  );
  let new_mod = apply_aliases(project.0.as_ref(), &map, i, injected_as);
  Ok(ProjectTree(Rc::new(new_mod)))
}