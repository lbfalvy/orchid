use std::iter;

use hashbrown::HashMap;
use intern_all::Tok;
use itertools::Itertools;

use super::walk_with_links::walk_with_links;
use crate::error::{ErrorPosition, ProjectError, ProjectResult};
use crate::location::CodeLocation;
use crate::name::{Sym, VPath};
use crate::parse::parsed::Expr;
use crate::pipeline::project::{
  ItemKind, ProjItem, ProjRule, ProjXEnt, ProjXMod, ProjectMod, SourceModule,
};
use crate::tree::{ModEntry, ModMember, Module};
use crate::utils::pure_seq::with_pushed;

#[derive(Clone)]
struct NotFound {
  location: CodeLocation,
  last_stop: VPath,
  bad_step: Tok<String>,
}
impl ProjectError for NotFound {
  const DESCRIPTION: &'static str = "A path pointed out of the tree";
  fn message(&self) -> String {
    format!("{} doesn't contain {}", self.last_stop, self.bad_step)
  }
  fn one_position(&self) -> CodeLocation { self.location.clone() }
}

struct NameErrors(Vec<NotFound>);
impl ProjectError for NameErrors {
  const DESCRIPTION: &'static str = "Some symbols were missing";
  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> {
    self.0.clone().into_iter().map(|nf| ErrorPosition {
      location: nf.one_position(),
      message: Some(nf.message()),
    })
  }
}

fn resolve_name(
  name: Sym,
  root: &ProjectMod,
  path: &[Tok<String>],
  location: CodeLocation,
  env: &Module<impl Sized, impl Sized, impl Sized>,
) -> Result<Option<Sym>, NotFound> {
  let full_name = path.iter().chain(&name[..]).cloned().collect_vec();
  match walk_with_links(root, full_name.clone().into_iter()) {
    Ok(rep) => Ok(Some(rep.abs_path.to_sym())),
    Err(mut e) => match e.tail.next() {
      // If it got stuck on the very last step, allow it through for
      // now in case it is a binding. If the name doesn't get bound, by
      // macros it will be raised at the postmacro check.
      None => Ok(Some(e.consumed_path().to_sym())),
      Some(step) => {
        // If there's more, rebuild the last full path after redirects and
        // try to resolve it on the env tree. The env tree doesn't contain
        // redirects so a plain tree walk is enough.
        let fallback_path = (e.abs_path.iter())
          .chain(iter::once(&e.name))
          .cloned()
          .chain(iter::once(step))
          .chain(e.tail)
          .collect_vec();
        let valid_in_env = env.walk1_ref(&[], &fallback_path, |_| true).is_ok();
        match valid_in_env {
          false => Err(NotFound {
            location,
            last_stop: VPath(e.abs_path),
            bad_step: e.name,
          }),
          true => Ok(e.aliased.then(|| {
            Sym::new(fallback_path).expect("Not empty by construction")
          })),
        }
      },
    },
  }
}

fn process_expr(
  expr: &Expr,
  root: &ProjectMod,
  path: &[Tok<String>],
  errors: &mut Vec<NotFound>,
  env: &Module<impl Sized, impl Sized, impl Sized>,
) -> Expr {
  let location = CodeLocation::Source(expr.range.clone());
  expr
    .map_names(&mut |n| {
      resolve_name(n, root, path, location.clone(), env).unwrap_or_else(|e| {
        errors.push(e);
        None
      })
    })
    .unwrap_or_else(|| expr.clone())
}

fn resolve_aliases_rec(
  root: &ProjectMod,
  path: &mut Vec<Tok<String>>,
  module: &ProjectMod,
  env: &Module<impl Sized, impl Sized, impl Sized>,
) -> ProjectResult<ProjectMod> {
  let mut errors = Vec::new();
  let module = Module {
    x: ProjXMod {
      src: module.x.src.as_ref().map(|s| SourceModule {
        range: s.range.clone(),
        rules: (s.rules.iter())
          .map(|ProjRule { pattern, prio, template, comments }| ProjRule {
            pattern: (pattern.iter())
              .map(|e| process_expr(e, root, path, &mut errors, env))
              .collect(),
            template: (template.iter())
              .map(|e| process_expr(e, root, path, &mut errors, env))
              .collect(),
            comments: comments.clone(),
            prio: *prio,
          })
          .collect(),
      }),
    },
    entries: (module.entries.iter())
      .map(|(k, v)| -> ProjectResult<_> {
        Ok((k.clone(), ModEntry {
          x: ProjXEnt {
            exported: v.x.exported,
            comments: v.x.comments.clone(),
            locations: v.x.locations.clone(),
          },
          member: match &v.member {
            ModMember::Sub(module) => {
              let m = with_pushed(path, k.clone(), |p| {
                resolve_aliases_rec(root, p, module, env)
              });
              ModMember::Sub(m.1?)
            },
            ModMember::Item(item) => ModMember::Item(ProjItem {
              kind: match &item.kind {
                ItemKind::Const(v) => {
                  let v = process_expr(v, root, path, &mut errors, env);
                  ItemKind::Const(v)
                },
                ItemKind::Alias(n) => {
                  let location = (v.x.locations.first().cloned())
                    .expect("Aliases are never created without source code");
                  // this is an absolute path so we set the path to empty
                  match resolve_name(n.clone(), root, &[], location, env) {
                    Ok(Some(n)) => ItemKind::Alias(n),
                    Ok(None) => ItemKind::Alias(n.clone()),
                    Err(e) => return Err(e.pack()),
                  }
                },
                _ => item.kind.clone(),
              },
            }),
          },
        }))
      })
      .collect::<Result<HashMap<_, _>, _>>()?,
  };
  errors.is_empty().then_some(module).ok_or_else(|| NameErrors(errors).pack())
}

pub fn resolve_aliases(
  project: ProjectMod,
  env: &Module<impl Sized, impl Sized, impl Sized>,
) -> ProjectResult<ProjectMod> {
  resolve_aliases_rec(&project, &mut Vec::new(), &project, env)
}
