use super::alias_map::AliasMap;
use super::decls::{InjectedAsFn, UpdatedFn};
use crate::ast::{Expr, Rule};
use crate::interner::Tok;
use crate::representations::project::{ItemKind, ProjectMod};
use crate::representations::tree::ModMember;
use crate::representations::VName;
use crate::utils::Substack;

fn resolve_rec(
  namespace: &[Tok<String>],
  alias_map: &AliasMap,
) -> Option<VName> {
  if let Some(alias) = alias_map.resolve(namespace) {
    Some(alias.clone())
  } else if let Some((foot, body)) = namespace.split_last() {
    let mut new_beginning = resolve_rec(body, alias_map)?;
    new_beginning.push(foot.clone());
    Some(new_beginning)
  } else {
    None
  }
}

fn resolve(
  namespace: &[Tok<String>],
  alias_map: &AliasMap,
  injected_as: &impl InjectedAsFn,
) -> Option<VName> {
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

/// Replace all aliases with the name they're originally defined as
fn apply_aliases_rec(
  path: Substack<Tok<String>>,
  module: &mut ProjectMod<VName>,
  alias_map: &AliasMap,
  injected_as: &impl InjectedAsFn,
  updated: &impl UpdatedFn,
) {
  for (name, entry) in module.entries.iter_mut() {
    match &mut entry.member {
      ModMember::Sub(sub) => {
        let subpath = path.push(name.clone());
        apply_aliases_rec(subpath, sub, alias_map, injected_as, updated)
      },
      ModMember::Item(it) => match &mut it.kind {
        ItemKind::None => (),
        ItemKind::Const(expr) =>
          *expr = process_expr(expr, alias_map, injected_as),
        ItemKind::Alias(name) =>
          if let Some(alt) = alias_map.resolve(&name) {
            *name = alt.clone()
          },
      },
      _ => (),
    }
  }
  for Rule { pattern, prio, template } in module.extra.rules.iter_mut() {
    for expr in pattern.iter_mut().chain(template.iter_mut()) {
      *expr = process_expr(expr, alias_map, injected_as)
    }
  }
}

pub fn apply_aliases(
  module: &mut ProjectMod<VName>,
  alias_map: &AliasMap,
  injected_as: &impl InjectedAsFn,
  updated: &impl UpdatedFn,
) {
  apply_aliases_rec(Substack::Bottom, module, alias_map, injected_as, updated)
}
