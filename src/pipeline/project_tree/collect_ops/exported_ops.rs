use std::println;
use std::rc::Rc;

use hashbrown::HashSet;
use itertools::Itertools;

use crate::representations::tree::WalkErrorKind;
use crate::pipeline::source_loader::LoadedSourceTable;
use crate::pipeline::error::{ProjectError, ModuleNotFound};
use crate::interner::{Token, Interner};
use crate::utils::Cache;
use crate::pipeline::split_name::split_name;

pub type OpsResult = Result<Rc<HashSet<Token<String>>>, Rc<dyn ProjectError>>;
pub type ExportedOpsCache<'a> = Cache<'a, Token<Vec<Token<String>>>, OpsResult>;

pub trait InjectedOperatorsFn = Fn(
  Token<Vec<Token<String>>>
) -> Option<Rc<HashSet<Token<String>>>>;

fn coprefix<T: Eq>(
  l: impl Iterator<Item = T>,
  r: impl Iterator<Item = T>
) -> usize {
  l.zip(r).take_while(|(a, b)| a == b).count()
}

/// Collect all names exported by the module at the specified path
pub fn collect_exported_ops(
  path: Token<Vec<Token<String>>>,
  loaded: &LoadedSourceTable,
  i: &Interner,
  injected: &impl InjectedOperatorsFn
) -> OpsResult {
  if let Some(ops) = injected(path) {
    if path == i.i(&[i.i("prelude")][..]) {
      println!("%%% Prelude exported ops %%%");
      println!("{}", ops.iter().map(|t| i.r(*t)).join(", "));
    }
    return Ok(ops)
  }
  let is_file = |n: &[Token<String>]| loaded.contains_key(&i.i(n));
  let path_s = &i.r(path)[..];
  let name_split = split_name(path_s, &is_file);
  let (fpath_v, subpath_v) = if let Some(f) = name_split {f} else {
    return Ok(Rc::new(loaded.keys().copied()
      .filter_map(|modname| {
        let modname_s = i.r(modname);
        if path_s.len() == coprefix(path_s.iter(), modname_s.iter()) {
          Some(modname_s[path_s.len()])
        } else {None}
      })
      .collect::<HashSet<_>>()
    ))
  };
  let fpath = i.i(fpath_v);
  let preparsed = &loaded[&fpath].preparsed;
  let module = preparsed.0.walk(&subpath_v, false)
    .map_err(|walk_err| match walk_err.kind {
      WalkErrorKind::Private => unreachable!("visibility is not being checked here"),
      WalkErrorKind::Missing => ModuleNotFound{
        file: i.extern_vec(fpath),
        subpath: subpath_v.into_iter()
          .take(walk_err.pos)
          .map(|t| i.r(*t))
          .cloned()
          .collect()
      }.rc(),
    })?;
  let out: HashSet<_> = module.items.iter()
    .filter(|(_, v)| v.exported)
    .map(|(k, _)| *k)
    .collect();
  if path == i.i(&[i.i("prelude")][..]) {
    println!("%%% Prelude exported ops %%%");
    println!("{}", out.iter().map(|t| i.r(*t)).join(", "));
  }
  Ok(Rc::new(out))
}

pub fn mk_cache<'a>(
  loaded: &'a LoadedSourceTable,
  i: &'a Interner,
  injected: &'a impl InjectedOperatorsFn,
) -> ExportedOpsCache<'a> {
  Cache::new(|path, _this| {
    collect_exported_ops(path, loaded, i, injected)
  })
}