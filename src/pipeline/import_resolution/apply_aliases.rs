use hashbrown::HashMap;

use super::alias_map::AliasMap;
use super::decls::{InjectedAsFn, UpdatedFn};
use crate::ast::{Expr, Rule};
use crate::interner::Tok;
use crate::pipeline::{ProjectExt, ProjectModule};
use crate::representations::tree::{ModEntry, ModMember};
use crate::representations::VName;
use crate::utils::Substack;

fn resolve_rec(
  namespace: &[Tok<String>],
  alias_map: &AliasMap,
) -> Option<Vec<Tok<String>>> {
  if let Some(alias) = alias_map.resolve(namespace) {
    Some(alias.clone())
  } else if let Some((foot, body)) = namespace.split_last() {
    let mut new_beginning = resolve_rec(body, alias_map)?;
    new_beginning.push(*foot);
    Some(new_beginning)
  } else {
    None
  }
}

fn resolve(
  namespace: &[Tok<String>],
  alias_map: &AliasMap,
  injected_as: &impl InjectedAsFn,
) -> Option<Vec<Tok<String>>> {
  injected_as(namespace).or_else(|| {
    let next_v = resolve_rec(namespace, alias_map)?;
    Some(injected_as(&next_v).unwrap_or(next_v))
  })
}

fn process_expr(
  expr: &Expr<VName>,
  alias_map: &AliasMap,
  injected_as: &impl InjectedAsFn,
) -> Expr<VName> {
  expr
    .map_names(&|n| resolve(n, alias_map, injected_as))
    .unwrap_or_else(|| expr.clone())
}

// TODO: replace is_injected with injected_as
/// Replace all aliases with the name they're originally defined as
fn apply_aliases_rec(
  path: Substack<Tok<String>>,
  module: &ProjectModule<VName>,
  alias_map: &AliasMap,
  injected_as: &impl InjectedAsFn,
  updated: &impl UpdatedFn,
) -> ProjectModule<VName> {
  let items = (module.items.iter())
    .map(|(name, ent)| {
      let ModEntry { exported, member } = ent;
      let member = match member {
        ModMember::Item(expr) =>
          ModMember::Item(process_expr(expr, alias_map, injected_as)),
        ModMember::Sub(module) => {
          let subpath = path.push(*name);
          let new_mod = if !updated(&subpath.iter().rev_vec_clone()) {
            module.clone()
          } else {
            apply_aliases_rec(subpath, module, alias_map, injected_as, updated)
          };
          ModMember::Sub(new_mod)
        },
      };
      (*name, ModEntry { exported: *exported, member })
    })
    .collect::<HashMap<_, _>>();
  let rules = (module.extra.rules.iter())
    .map(|rule| {
      let Rule { pattern, prio, template } = rule;
      Rule {
        prio: *prio,
        pattern: (pattern.iter())
          .map(|expr| process_expr(expr, alias_map, injected_as))
          .collect::<Vec<_>>(),
        template: (template.iter())
          .map(|expr| process_expr(expr, alias_map, injected_as))
          .collect::<Vec<_>>(),
      }
    })
    .collect::<Vec<_>>();
  ProjectModule {
    items,
    imports: module.imports.clone(),
    extra: ProjectExt {
      rules,
      exports: (module.extra.exports.iter())
        .map(|(k, v)| {
          (*k, resolve(v, alias_map, injected_as).unwrap_or(v.clone()))
        })
        .collect(),
      file: module.extra.file.clone(),
      imports_from: module.extra.imports_from.clone(),
    },
  }
}

pub fn apply_aliases(
  module: &ProjectModule<VName>,
  alias_map: &AliasMap,
  injected_as: &impl InjectedAsFn,
  updated: &impl UpdatedFn,
) -> ProjectModule<VName> {
  apply_aliases_rec(Substack::Bottom, module, alias_map, injected_as, updated)
}
