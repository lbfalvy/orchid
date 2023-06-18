use core::panic;
use std::rc::Rc;

use super::alias_map::AliasMap;
use super::decls::UpdatedFn;
use crate::interner::{Interner, Tok};
use crate::pipeline::error::{NotExported, ProjectError};
use crate::pipeline::project_tree::{split_path, ProjectModule, ProjectTree};
use crate::representations::tree::{ModMember, WalkErrorKind};
use crate::representations::VName;
use crate::utils::{pushed, unwrap_or, Substack};

/// Assert that a module identified by a path can see a given symbol
fn assert_visible(
  source: &[Tok<String>], // must point to a file or submodule
  target: &[Tok<String>], // may point to a symbol or module of any kind
  project: &ProjectTree<VName>,
  i: &Interner,
) -> Result<(), Rc<dyn ProjectError>> {
  let (tgt_item, tgt_path) = unwrap_or!(target.split_last(); return Ok(()));
  let shared_len =
    source.iter().zip(tgt_path.iter()).take_while(|(a, b)| a == b).count();
  let vis_ignored_len = usize::min(tgt_path.len(), shared_len + 1);
  let private_root = (project.0)
    .walk_ref(&tgt_path[..vis_ignored_len], false)
    .unwrap_or_else(|e| {
      let path_slc = &tgt_path[..vis_ignored_len];
      let bad_path = i.extern_all(path_slc).join("::");
      eprintln!(
        "Error while walking {bad_path}; {:?} on step {}",
        e.kind, e.pos
      );
      eprintln!("looking from {}", i.extern_all(source).join("::"));
      panic!("")
    });
  let direct_parent = private_root
    .walk_ref(&tgt_path[vis_ignored_len..], true)
    .map_err(|e| match e.kind {
      WalkErrorKind::Missing => panic!("checked in parsing"),
      WalkErrorKind::Private => {
        let full_path = &tgt_path[..shared_len + e.pos];
        let (file, sub) = split_path(full_path, project);
        let (ref_file, ref_sub) = split_path(source, project);
        NotExported {
          file: i.extern_all(file),
          subpath: i.extern_all(sub),
          referrer_file: i.extern_all(ref_file),
          referrer_subpath: i.extern_all(ref_sub),
        }
        .rc()
      },
    })?;
  let tgt_item_exported = direct_parent.extra.exports.contains_key(tgt_item);
  let target_prefixes_source = shared_len == tgt_path.len();
  if !tgt_item_exported && !target_prefixes_source {
    let (file, sub) = split_path(target, project);
    let (ref_file, ref_sub) = split_path(source, project);
    Err(
      NotExported {
        file: i.extern_all(file),
        subpath: i.extern_all(sub),
        referrer_file: i.extern_all(ref_file),
        referrer_subpath: i.extern_all(ref_sub),
      }
      .rc(),
    )
  } else {
    Ok(())
  }
}

/// Populate target and alias maps from the module tree recursively
fn collect_aliases_rec(
  path: Substack<Tok<String>>,
  module: &ProjectModule<VName>,
  project: &ProjectTree<VName>,
  alias_map: &mut AliasMap,
  i: &Interner,
  updated: &impl UpdatedFn,
) -> Result<(), Rc<dyn ProjectError>> {
  // Assume injected module has been alias-resolved
  let mod_path_v = path.iter().rev_vec_clone();
  if !updated(&mod_path_v) {
    return Ok(());
  };
  for (&name, target_mod_name) in module.extra.imports_from.iter() {
    let target_sym_v = pushed(target_mod_name, name);
    assert_visible(&mod_path_v, &target_sym_v, project, i)?;
    let sym_path_v = pushed(&mod_path_v, name);
    let target_mod = (project.0.walk_ref(target_mod_name, false))
      .expect("checked above in assert_visible");
    let target_sym = target_mod
      .extra
      .exports
      .get(&name)
      .unwrap_or_else(|| {
        panic!(
          "error in {}, {} has no member {}",
          i.extern_all(&mod_path_v).join("::"),
          i.extern_all(target_mod_name).join("::"),
          i.r(name)
        )
      })
      .clone();
    alias_map.link(sym_path_v, target_sym);
  }
  for (&name, entry) in module.items.iter() {
    let submodule = unwrap_or!(&entry.member => ModMember::Sub; continue);
    collect_aliases_rec(
      path.push(name),
      submodule,
      project,
      alias_map,
      i,
      updated,
    )?
  }
  Ok(())
}

/// Populate target and alias maps from the module tree
pub fn collect_aliases(
  module: &ProjectModule<VName>,
  project: &ProjectTree<VName>,
  alias_map: &mut AliasMap,
  i: &Interner,
  updated: &impl UpdatedFn,
) -> Result<(), Rc<dyn ProjectError>> {
  collect_aliases_rec(Substack::Bottom, module, project, alias_map, i, updated)
}
