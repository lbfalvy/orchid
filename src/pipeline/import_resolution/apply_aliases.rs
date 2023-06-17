use std::rc::Rc;

use hashbrown::HashMap;

use super::alias_map::AliasMap;
use super::decls::{InjectedAsFn, UpdatedFn};
use crate::ast::{Expr, Rule};
use crate::interner::{Interner, Sym, Tok};
use crate::pipeline::{ProjectExt, ProjectModule};
use crate::representations::tree::{ModEntry, ModMember};
use crate::utils::Substack;

fn resolve_rec(
  token: Sym,
  alias_map: &AliasMap,
  i: &Interner,
) -> Option<Vec<Tok<String>>> {
  if let Some(alias) = alias_map.resolve(token) {
    Some(i.r(alias).clone())
  } else if let Some((foot, body)) = i.r(token).split_last() {
    let mut new_beginning = resolve_rec(i.i(body), alias_map, i)?;
    new_beginning.push(*foot);
    Some(new_beginning)
  } else {
    None
  }
}

fn resolve(
  token: Sym,
  alias_map: &AliasMap,
  injected_as: &impl InjectedAsFn,
  i: &Interner,
) -> Option<Sym> {
  injected_as(&i.r(token)[..]).or_else(|| {
    let next_v = resolve_rec(token, alias_map, i)?;
    Some(injected_as(&next_v).unwrap_or_else(|| i.i(&next_v)))
  })
}

fn process_expr(
  expr: &Expr,
  alias_map: &AliasMap,
  injected_as: &impl InjectedAsFn,
  i: &Interner,
) -> Expr {
  expr
    .map_names(&|n| resolve(n, alias_map, injected_as, i))
    .unwrap_or_else(|| expr.clone())
}

// TODO: replace is_injected with injected_as
/// Replace all aliases with the name they're originally defined as
fn apply_aliases_rec(
  path: Substack<Tok<String>>,
  module: &ProjectModule,
  alias_map: &AliasMap,
  i: &Interner,
  injected_as: &impl InjectedAsFn,
  updated: &impl UpdatedFn,
) -> ProjectModule {
  let items = (module.items.iter())
    .map(|(name, ent)| {
      let ModEntry { exported, member } = ent;
      let member = match member {
        ModMember::Item(expr) =>
          ModMember::Item(process_expr(expr, alias_map, injected_as, i)),
        ModMember::Sub(module) => {
          let subpath = path.push(*name);
          let new_mod = if !updated(&subpath.iter().rev_vec_clone()) {
            module.clone()
          } else {
            let module = module.as_ref();
            Rc::new(apply_aliases_rec(
              subpath,
              module,
              alias_map,
              i,
              injected_as,
              updated,
            ))
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
        pattern: Rc::new(
          (pattern.iter())
            .map(|expr| process_expr(expr, alias_map, injected_as, i))
            .collect::<Vec<_>>(),
        ),
        template: Rc::new(
          (template.iter())
            .map(|expr| process_expr(expr, alias_map, injected_as, i))
            .collect::<Vec<_>>(),
        ),
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
          (*k, resolve(*v, alias_map, injected_as, i).unwrap_or(*v))
        })
        .collect(),
      file: module.extra.file.clone(),
      imports_from: module.extra.imports_from.clone(),
    },
  }
}

pub fn apply_aliases(
  module: &ProjectModule,
  alias_map: &AliasMap,
  i: &Interner,
  injected_as: &impl InjectedAsFn,
  updated: &impl UpdatedFn,
) -> ProjectModule {
  apply_aliases_rec(
    Substack::Bottom,
    module,
    alias_map,
    i,
    injected_as,
    updated,
  )
}
