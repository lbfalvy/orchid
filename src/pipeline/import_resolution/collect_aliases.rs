use std::rc::Rc;

use super::alias_map::AliasMap;
use super::decls::InjectedAsFn;
use crate::interner::{Interner, Tok};
use crate::pipeline::error::{NotExported, ProjectError};
use crate::pipeline::project_tree::{split_path, ProjectModule, ProjectTree};
use crate::representations::tree::{ModMember, WalkErrorKind};
use crate::utils::{pushed, Substack};

/// Assert that a module identified by a path can see a given symbol
fn assert_visible(
  source: &[Tok<String>], // must point to a file or submodule
  target: &[Tok<String>], // may point to a symbol or module of any kind
  project: &ProjectTree,
  i: &Interner,
) -> Result<(), Rc<dyn ProjectError>> {
  let (tgt_item, tgt_path) = if let Some(s) = target.split_last() {
    s
  } else {
    return Ok(());
  };
  let shared_len =
    source.iter().zip(tgt_path.iter()).take_while(|(a, b)| a == b).count();
  let shared_root =
    project.0.walk(&tgt_path[..shared_len], false).expect("checked in parsing");
  let direct_parent =
    shared_root.walk(&tgt_path[shared_len..], true).map_err(|e| {
      match e.kind {
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
      }
    })?;
  let tgt_item_exported = direct_parent.extra.exports.contains_key(tgt_item);
  let target_prefixes_source =
    shared_len == tgt_path.len() && source.get(shared_len) == Some(tgt_item);
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
  module: &ProjectModule,
  project: &ProjectTree,
  alias_map: &mut AliasMap,
  i: &Interner,
  injected_as: &impl InjectedAsFn,
) -> Result<(), Rc<dyn ProjectError>> {
  // Assume injected module has been alias-resolved
  let mod_path_v = path.iter().rev_vec_clone();
  if injected_as(&mod_path_v).is_some() {
    return Ok(());
  };
  for (&name, &target_mod) in module.extra.imports_from.iter() {
    let target_mod_v = i.r(target_mod);
    let target_sym_v = pushed(target_mod_v, name);
    assert_visible(&mod_path_v, &target_sym_v, project, i)?;
    let sym_path_v = pushed(&mod_path_v, name);
    let sym_path = i.i(&sym_path_v);
    let target_sym = i.i(&target_sym_v);
    alias_map.link(sym_path, target_sym);
  }
  for (&name, entry) in module.items.iter() {
    let submodule = if let ModMember::Sub(s) = &entry.member {
      s.as_ref()
    } else {
      continue;
    };
    collect_aliases_rec(
      path.push(name),
      submodule,
      project,
      alias_map,
      i,
      injected_as,
    )?
  }
  Ok(())
}

/// Populate target and alias maps from the module tree
pub fn collect_aliases(
  module: &ProjectModule,
  project: &ProjectTree,
  alias_map: &mut AliasMap,
  i: &Interner,
  injected_as: &impl InjectedAsFn,
) -> Result<(), Rc<dyn ProjectError>> {
  collect_aliases_rec(
    Substack::Bottom,
    module,
    project,
    alias_map,
    i,
    injected_as,
  )
}
