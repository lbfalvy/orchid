use std::rc::Rc;

use hashbrown::HashMap;

use crate::{utils::Substack, interner::{Token, Interner}, pipeline::{ProjectModule, ProjectExt}, representations::tree::{ModEntry, ModMember}, ast::{Rule, Expr}};

use super::{alias_map::AliasMap, decls::InjectedAsFn};

fn process_expr(
  expr: &Expr,
  alias_map: &AliasMap,
  injected_as: &impl InjectedAsFn,
  i: &Interner,
) -> Expr {
  expr.map_names(&|n| {
    injected_as(&i.r(n)[..]).or_else(|| {
      alias_map.resolve(n).map(|n| {
        injected_as(&i.r(n)[..]).unwrap_or(n)
      })
    })
  }).unwrap_or_else(|| expr.clone())
}

// TODO: replace is_injected with injected_as
/// Replace all aliases with the name they're originally defined as
fn apply_aliases_rec(
  path: Substack<Token<String>>,
  module: &ProjectModule,
  alias_map: &AliasMap,
  i: &Interner,
  injected_as: &impl InjectedAsFn,
) -> ProjectModule {
  let items = module.items.iter().map(|(name, ent)| {
    let ModEntry{ exported, member } = ent;
    let member = match member {
      ModMember::Item(expr) => ModMember::Item(
        process_expr(expr, alias_map, injected_as, i)
      ),
      ModMember::Sub(module) => {
        let subpath = path.push(*name);
        let is_ignored = injected_as(&subpath.iter().rev_vec_clone()).is_some();
        let new_mod = if is_ignored {module.clone()} else {
          let module = module.as_ref();
          Rc::new(apply_aliases_rec(
            subpath, module,
            alias_map, i, injected_as
          ))
        };
        ModMember::Sub(new_mod)
      }
    };
    (*name, ModEntry{ exported: *exported, member })
  }).collect::<HashMap<_, _>>();
  let rules = module.extra.rules.iter().map(|rule| {
    let Rule{ source, prio, target } = rule;
    Rule{
      prio: *prio,
      source: Rc::new(source.iter()
        .map(|expr| process_expr(expr, alias_map, injected_as, i))
        .collect::<Vec<_>>()
      ),
      target: Rc::new(target.iter()
        .map(|expr| process_expr(expr, alias_map, injected_as, i))
        .collect::<Vec<_>>()
      ),
    }
  }).collect::<Vec<_>>();
  ProjectModule{
    items,
    imports: module.imports.clone(),
    extra: ProjectExt{
      rules,
      exports: module.extra.exports.clone(),
      file: module.extra.file.clone(),
      imports_from: module.extra.imports_from.clone(),
    }
  }
}

pub fn apply_aliases(
  module: &ProjectModule,
  alias_map: &AliasMap,
  i: &Interner,
  injected_as: &impl InjectedAsFn,
) -> ProjectModule {
  apply_aliases_rec(Substack::Bottom, module, alias_map, i, injected_as)
}