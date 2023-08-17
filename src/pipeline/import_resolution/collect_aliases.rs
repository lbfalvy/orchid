use super::alias_map::AliasMap;
use super::decls::UpdatedFn;
use crate::error::{NotExported, NotFound, ProjectError, ProjectResult};
use crate::interner::Tok;
use crate::pipeline::project_tree::split_path;
use crate::representations::project::{ProjectModule, ProjectTree};
use crate::representations::tree::{ModMember, WalkErrorKind};
use crate::representations::VName;
use crate::utils::{pushed, unwrap_or, Substack};

/// Assert that a module identified by a path can see a given symbol
fn assert_visible(
  source: &[Tok<String>], // must point to a file or submodule
  target: &[Tok<String>], // may point to a symbol or module of any kind
  project: &ProjectTree<VName>,
) -> ProjectResult<()> {
  let (tgt_item, tgt_path) = unwrap_or!(target.split_last(); return Ok(()));
  let shared_len =
    source.iter().zip(tgt_path.iter()).take_while(|(a, b)| a == b).count();
  let vis_ignored_len = usize::min(tgt_path.len(), shared_len + 1);
  let private_root = (project.0)
    .walk_ref(&tgt_path[..vis_ignored_len], false)
    .map_err(|e| match e.kind {
      WalkErrorKind::Private =>
        unreachable!("visibility is not being checked here"),
      WalkErrorKind::Missing => NotFound::from_walk_error(
        source,
        &[],
        &tgt_path[..vis_ignored_len],
        &project.0,
        e,
      )
      .rc(),
    })?;
  let direct_parent = private_root
    .walk_ref(&tgt_path[vis_ignored_len..], true)
    .map_err(|e| match e.kind {
      WalkErrorKind::Missing => NotFound::from_walk_error(
        source,
        &tgt_path[..vis_ignored_len],
        &tgt_path[vis_ignored_len..],
        &project.0,
        e,
      )
      .rc(),
      WalkErrorKind::Private => {
        let full_path = &tgt_path[..shared_len + e.pos];
        // These errors are encountered during error reporting but they're more
        // fundamental / higher prio than the error to be raised and would
        // emerge nonetheless so they take over and the original error is
        // swallowed
        match split_path(full_path, project) {
          Err(e) =>
            NotFound::from_walk_error(source, &[], full_path, &project.0, e)
              .rc(),
          Ok((file, sub)) => {
            let (ref_file, ref_sub) = split_path(source, project)
              .expect("Source path assumed to be valid");
            NotExported {
              file: file.to_vec(),
              subpath: sub.to_vec(),
              referrer_file: ref_file.to_vec(),
              referrer_subpath: ref_sub.to_vec(),
            }
            .rc()
          },
        }
      },
    })?;
  let tgt_item_exported = direct_parent.extra.exports.contains_key(tgt_item);
  let target_prefixes_source = shared_len == tgt_path.len();
  if !tgt_item_exported && !target_prefixes_source {
    let (file, sub) = split_path(target, project).map_err(|e| {
      NotFound::from_walk_error(source, &[], target, &project.0, e).rc()
    })?;
    let (ref_file, ref_sub) = split_path(source, project)
      .expect("The source path is assumed to be valid");
    Err(
      NotExported {
        file: file.to_vec(),
        subpath: sub.to_vec(),
        referrer_file: ref_file.to_vec(),
        referrer_subpath: ref_sub.to_vec(),
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
  updated: &impl UpdatedFn,
) -> ProjectResult<()> {
  // Assume injected module has been alias-resolved
  let mod_path_v = path.iter().rev_vec_clone();
  if !updated(&mod_path_v) {
    return Ok(());
  };
  for (&name, target_mod_name) in module.extra.imports_from.iter() {
    let target_sym_v = pushed(target_mod_name, name);
    assert_visible(&mod_path_v, &target_sym_v, project)?;
    let sym_path_v = pushed(&mod_path_v, name);
    let target_mod = (project.0.walk_ref(target_mod_name, false))
      .expect("checked above in assert_visible");
    let target_sym = (target_mod.extra.exports.get(&name))
      .ok_or_else(|| {
        let file_len =
          target_mod.extra.file.as_ref().unwrap_or(target_mod_name).len();
        NotFound {
          source: Some(mod_path_v.clone()),
          file: target_mod_name[..file_len].to_vec(),
          subpath: target_sym_v[file_len..].to_vec(),
        }
        .rc()
      })?
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
  updated: &impl UpdatedFn,
) -> ProjectResult<()> {
  collect_aliases_rec(Substack::Bottom, module, project, alias_map, updated)
}
