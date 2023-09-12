use super::alias_map::AliasMap;
use super::decls::UpdatedFn;
use crate::error::ProjectResult;
use crate::interner::Tok;
use crate::representations::project::{ProjectMod, ProjectTree};
use crate::representations::tree::ModMember;
use crate::representations::VName;
use crate::utils::{pushed, unwrap_or};

/// Populate target and alias maps from the module tree recursively
fn collect_aliases_rec(
  path: Vec<Tok<String>>,
  module: &ProjectMod<VName>,
  project: &ProjectTree<VName>,
  alias_map: &mut AliasMap,
  updated: &impl UpdatedFn,
) -> ProjectResult<()> {
  // Assume injected module has been alias-resolved
  if !updated(&path) {
    return Ok(());
  };
  for (name, target_sym_v) in module.extra.imports_from.iter() {
    let sym_path_v = pushed(&path, name.clone());
    alias_map.link(sym_path_v, target_sym_v.clone());
  }
  for (name, entry) in module.entries.iter() {
    let submodule = unwrap_or!(&entry.member => ModMember::Sub; continue);
    collect_aliases_rec(
      pushed(&path, name.clone()),
      submodule,
      project,
      alias_map,
      updated,
    )?
  }
  Ok(())
}

/// Populate target and alias maps from the module tree
pub fn collect_aliases(
  module: &ProjectMod<VName>,
  project: &ProjectTree<VName>,
  alias_map: &mut AliasMap,
  updated: &impl UpdatedFn,
) -> ProjectResult<()> {
  collect_aliases_rec(Vec::new(), module, project, alias_map, updated)
}
