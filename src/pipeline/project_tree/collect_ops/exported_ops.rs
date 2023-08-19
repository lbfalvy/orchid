use std::rc::Rc;

use hashbrown::HashSet;
use trait_set::trait_set;

use crate::error::{NotFound, ProjectError, ProjectResult};
use crate::interner::Tok;
use crate::pipeline::source_loader::LoadedSourceTable;
use crate::representations::tree::WalkErrorKind;
use crate::utils::{split_max_prefix, Cache};
use crate::Sym;

pub type OpsResult = ProjectResult<Rc<HashSet<Tok<String>>>>;
pub type ExportedOpsCache<'a> = Cache<'a, Sym, OpsResult>;

trait_set! {
  pub trait InjectedOperatorsFn = Fn(Sym) -> Option<Rc<HashSet<Tok<String>>>>;
}

fn coprefix<T: Eq>(
  l: impl Iterator<Item = T>,
  r: impl Iterator<Item = T>,
) -> usize {
  l.zip(r).take_while(|(a, b)| a == b).count()
}

/// Collect all names exported by the module at the specified path
pub fn collect_exported_ops(
  path: Sym,
  loaded: &LoadedSourceTable,
  injected: &impl InjectedOperatorsFn,
) -> OpsResult {
  let injected =
    injected(path.clone()).unwrap_or_else(|| Rc::new(HashSet::new()));
  match split_max_prefix(&path, &|n| loaded.contains_key(n)) {
    None => {
      let ops = (loaded.keys())
        .filter_map(|modname| {
          if path.len() == coprefix(path.iter(), modname.iter()) {
            Some(modname[path.len()].clone())
          } else {
            None
          }
        })
        .chain(injected.iter().cloned())
        .collect::<HashSet<_>>();
      Ok(Rc::new(ops))
    },
    Some((fpath, subpath)) => {
      let preparsed = &loaded[fpath].preparsed;
      let module = preparsed.0.walk_ref(subpath, false).map_err(
        |walk_err| match walk_err.kind {
          WalkErrorKind::Private => {
            unreachable!("visibility is not being checked here")
          },
          WalkErrorKind::Missing => NotFound {
            source: None,
            file: fpath.to_vec(),
            subpath: subpath[..walk_err.pos].to_vec(),
          }
          .rc(),
        },
      )?;
      let out = (module.items.iter())
        .filter(|(_, v)| v.exported)
        .map(|(k, _)| k.clone())
        .chain(injected.iter().cloned())
        .collect::<HashSet<_>>();
      Ok(Rc::new(out))
    },
  }
}

pub fn mk_cache<'a>(
  loaded: &'a LoadedSourceTable,
  injected: &'a impl InjectedOperatorsFn,
) -> ExportedOpsCache<'a> {
  Cache::new(|path, _this| collect_exported_ops(path, loaded, injected))
}
