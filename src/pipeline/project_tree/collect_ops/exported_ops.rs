use std::rc::Rc;

use hashbrown::HashSet;
use trait_set::trait_set;

use crate::interner::{Interner, Sym, Tok};
use crate::pipeline::error::{ModuleNotFound, ProjectError};
use crate::pipeline::source_loader::LoadedSourceTable;
use crate::representations::tree::WalkErrorKind;
use crate::utils::{split_max_prefix, unwrap_or, Cache};

pub type OpsResult = Result<Rc<HashSet<Tok<String>>>, Rc<dyn ProjectError>>;
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
  i: &Interner,
  injected: &impl InjectedOperatorsFn,
) -> OpsResult {
  let injected = injected(path).unwrap_or_else(|| Rc::new(HashSet::new()));
  let is_file = |n: &[Tok<String>]| loaded.contains_key(&i.i(n));
  let path_s = &i.r(path)[..];
  let name_split = split_max_prefix(path_s, &is_file);
  let (fpath_v, subpath_v) = unwrap_or!(name_split; return Ok(Rc::new(
    (loaded.keys())
      .copied()
      .filter_map(|modname| {
        let modname_s = i.r(modname);
        if path_s.len() == coprefix(path_s.iter(), modname_s.iter()) {
          Some(modname_s[path_s.len()])
        } else {
          None
        }
      })
      .chain(injected.iter().copied())
      .collect::<HashSet<_>>(),
  )));
  let fpath = i.i(fpath_v);
  let preparsed = &loaded[&fpath].preparsed;
  let module = preparsed.0.walk(subpath_v, false).map_err(|walk_err| {
    match walk_err.kind {
      WalkErrorKind::Private => {
        unreachable!("visibility is not being checked here")
      },
      WalkErrorKind::Missing => ModuleNotFound {
        file: i.extern_vec(fpath),
        subpath: (subpath_v.iter())
          .take(walk_err.pos)
          .map(|t| i.r(*t))
          .cloned()
          .collect(),
      }
      .rc(),
    }
  })?;
  let out = (module.items.iter())
    .filter(|(_, v)| v.exported)
    .map(|(k, _)| *k)
    .chain(injected.iter().copied())
    .collect::<HashSet<_>>();
  Ok(Rc::new(out))
}

pub fn mk_cache<'a>(
  loaded: &'a LoadedSourceTable,
  i: &'a Interner,
  injected: &'a impl InjectedOperatorsFn,
) -> ExportedOpsCache<'a> {
  Cache::new(|path, _this| collect_exported_ops(path, loaded, i, injected))
}
