use std::iter;

use itertools::Itertools;

use super::walk_with_links::walk_with_links;
use crate::ast::{Expr, Rule};
use crate::representations::project::{
  ItemKind, ProjectExt, ProjectItem, ProjectMod,
};
use crate::tree::{ModEntry, ModMember, Module};
use crate::utils::pure_seq::pushed;
use crate::{Interner, ProjectTree, Tok, VName};

#[must_use]
fn resolve_aliases_rec(
  root: &ProjectMod<VName>,
  module: &ProjectMod<VName>,
  updated: &impl Fn(&[Tok<String>]) -> bool,
  is_root: bool,
) -> ProjectMod<VName> {
  if !is_root && !updated(&module.extra.path) {
    return module.clone();
  }
  let process_expr = |expr: &Expr<VName>| {
    expr
      .map_names(&|n| {
        let full_name = (module.extra.path.iter()).chain(n.iter()).cloned();
        match walk_with_links(root, full_name, false) {
          Ok(rep) => Some(rep.abs_path),
          Err(e) => {
            let leftovers = e.tail.collect::<Vec<_>>();
            if !leftovers.is_empty() {
              let full_name = (module.extra.path.iter())
                .chain(n.iter())
                .cloned()
                .collect::<Vec<_>>();
              let _ = walk_with_links(root, full_name.iter().cloned(), true);
              panic!(
                "Invalid path {} while resolving {} should have been noticed \
                 earlier",
                (e.abs_path.into_iter())
                  .chain(iter::once(e.name))
                  .chain(leftovers.into_iter())
                  .join("::"),
                Interner::extern_all(&full_name).join("::"),
              );
            }
            Some(pushed(e.abs_path, e.name))
          },
        }
      })
      .unwrap_or_else(|| expr.clone())
  };
  Module {
    extra: ProjectExt {
      path: module.extra.path.clone(),
      file: module.extra.file.clone(),
      imports_from: module.extra.imports_from.clone(),
      rules: (module.extra.rules.iter())
        .map(|Rule { pattern, prio, template }| Rule {
          pattern: pattern.iter().map(process_expr).collect(),
          template: template.iter().map(process_expr).collect(),
          prio: *prio,
        })
        .collect(),
    },
    entries: module
      .entries
      .iter()
      .map(|(k, v)| {
        (k.clone(), ModEntry {
          exported: v.exported,
          member: match &v.member {
            ModMember::Sub(module) =>
              ModMember::Sub(resolve_aliases_rec(root, module, updated, false)),
            ModMember::Item(item) => ModMember::Item(ProjectItem {
              kind: match &item.kind {
                ItemKind::Const(value) => ItemKind::Const(process_expr(value)),
                other => other.clone(),
              },
            }),
          },
        })
      })
      .collect(),
  }
}

#[must_use]
pub fn resolve_aliases(
  project: ProjectTree<VName>,
  updated: &impl Fn(&[Tok<String>]) -> bool,
) -> ProjectTree<VName> {
  ProjectTree(resolve_aliases_rec(&project.0, &project.0, updated, true))
}
