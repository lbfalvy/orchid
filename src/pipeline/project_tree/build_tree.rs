use std::rc::Rc;

use hashbrown::HashMap;
use itertools::Itertools;

use super::collect_ops::InjectedOperatorsFn;
use super::parse_file::parse_file;
use super::{collect_ops, ProjectExt, ProjectTree};
use crate::ast::{Constant, Expr};
use crate::interner::{Interner, Tok};
use crate::pipeline::error::ProjectError;
use crate::pipeline::source_loader::{LoadedSource, LoadedSourceTable};
use crate::representations::sourcefile::{absolute_path, FileEntry, Member};
use crate::representations::tree::{ModEntry, ModMember, Module};
use crate::representations::{NameLike, VName};
use crate::utils::iter::{box_empty, box_once};
use crate::utils::{pushed, unwrap_or, Substack};

#[derive(Debug)]
struct ParsedSource<'a> {
  path: Vec<Tok<String>>,
  loaded: &'a LoadedSource,
  parsed: Vec<FileEntry>,
}

pub fn split_path<'a>(
  path: &'a [Tok<String>],
  proj: &'a ProjectTree<impl NameLike>,
) -> (&'a [Tok<String>], &'a [Tok<String>]) {
  let (end, body) = if let Some(s) = path.split_last() {
    s
  } else {
    return (&[], &[]);
  };
  let mut module =
    proj.0.walk_ref(body, false).expect("invalid path cannot be split");
  if let ModMember::Sub(m) = &module.items[end].member {
    module = m;
  }
  let file =
    module.extra.file.as_ref().map(|s| &path[..s.len()]).unwrap_or(path);
  let subpath = &path[file.len()..];
  (file, subpath)
}

/// Convert normalized, prefixed source into a module
fn source_to_module(
  // level
  path: Substack<Tok<String>>,
  preparsed: &Module<impl Clone, impl Clone>,
  // data
  data: Vec<FileEntry>,
  // context
  i: &Interner,
  filepath_len: usize,
) -> Module<Expr<VName>, ProjectExt<VName>> {
  let path_v = path.iter().rev_vec_clone();
  let imports = data
    .iter()
    .filter_map(|ent| {
      if let FileEntry::Import(impv) = ent { Some(impv.iter()) } else { None }
    })
    .flatten()
    .cloned()
    .collect::<Vec<_>>();
  let imports_from = imports
    .iter()
    .map(|imp| {
      let mut imp_path_v = i.r(imp.path).clone();
      imp_path_v.push(imp.name.expect("imports normalized"));
      let mut abs_path =
        absolute_path(&path_v, &imp_path_v, i).expect("tested in preparsing");
      let name = abs_path.pop().expect("importing the global context");
      (name, abs_path)
    })
    .collect::<HashMap<_, _>>();
  let exports = data
    .iter()
    .flat_map(|ent| {
      let mk_ent = |name| (name, pushed(&path_v, name));
      match ent {
        FileEntry::Export(names) => Box::new(names.iter().copied().map(mk_ent)),
        FileEntry::Exported(mem) => match mem {
          Member::Constant(constant) => box_once(mk_ent(constant.name)),
          Member::Namespace(ns) => box_once(mk_ent(ns.name)),
          Member::Rule(rule) => {
            let mut names = Vec::new();
            for e in rule.pattern.iter() {
              e.visit_names(Substack::Bottom, &mut |n| {
                if let Some([name]) = n.strip_prefix(&path_v[..]) {
                  names.push((*name, n.clone()))
                }
              })
            }
            Box::new(names.into_iter())
          },
        },
        _ => box_empty(),
      }
    })
    .collect::<HashMap<_, _>>();
  let rules = data
    .iter()
    .filter_map(|ent| match ent {
      FileEntry::Exported(Member::Rule(rule)) => Some(rule),
      FileEntry::Internal(Member::Rule(rule)) => Some(rule),
      _ => None,
    })
    .cloned()
    .collect::<Vec<_>>();
  let items = data
    .into_iter()
    .filter_map(|ent| {
      let member_to_item = |exported, member| match member {
        Member::Namespace(ns) => {
          let new_prep = unwrap_or!(
            &preparsed.items[&ns.name].member => ModMember::Sub;
            panic!("preparsed missing a submodule")
          );
          let module = source_to_module(
            path.push(ns.name),
            new_prep,
            ns.body,
            i,
            filepath_len,
          );
          let member = ModMember::Sub(module);
          Some((ns.name, ModEntry { exported, member }))
        },
        Member::Constant(Constant { name, value }) => {
          let member = ModMember::Item(value);
          Some((name, ModEntry { exported, member }))
        },
        _ => None,
      };
      match ent {
        FileEntry::Exported(member) => member_to_item(true, member),
        FileEntry::Internal(member) => member_to_item(false, member),
        _ => None,
      }
    })
    .collect::<HashMap<_, _>>();
  Module {
    imports,
    items,
    extra: ProjectExt {
      imports_from,
      exports,
      rules,
      file: Some(path_v[..filepath_len].to_vec()),
    },
  }
}

fn files_to_module(
  path: Substack<Tok<String>>,
  files: Vec<ParsedSource>,
  i: &Interner,
) -> Module<Expr<VName>, ProjectExt<VName>> {
  let lvl = path.len();
  debug_assert!(
    files.iter().map(|f| f.path.len()).max().unwrap() >= lvl,
    "path is longer than any of the considered file paths"
  );
  let path_v = path.iter().rev_vec_clone();
  if files.len() == 1 && files[0].path.len() == lvl {
    return source_to_module(
      path,
      &files[0].loaded.preparsed.0,
      files[0].parsed.clone(),
      i,
      path.len(),
    );
  }
  let items = files
    .into_iter()
    .group_by(|f| f.path[lvl])
    .into_iter()
    .map(|(namespace, files)| {
      let subpath = path.push(namespace);
      let files_v = files.collect::<Vec<_>>();
      let module = files_to_module(subpath, files_v, i);
      let member = ModMember::Sub(module);
      (namespace, ModEntry { exported: true, member })
    })
    .collect::<HashMap<_, _>>();
  let exports: HashMap<_, _> =
    items.keys().copied().map(|name| (name, pushed(&path_v, name))).collect();
  Module {
    items,
    imports: vec![],
    extra: ProjectExt {
      exports,
      imports_from: HashMap::new(),
      rules: vec![],
      file: None,
    },
  }
}

pub fn build_tree(
  files: LoadedSourceTable,
  i: &Interner,
  prelude: &[FileEntry],
  injected: &impl InjectedOperatorsFn,
) -> Result<ProjectTree<VName>, Rc<dyn ProjectError>> {
  assert!(!files.is_empty(), "A tree requires at least one module");
  let ops_cache = collect_ops::mk_cache(&files, i, injected);
  let mut entries = files
    .iter()
    .map(|(path, loaded)| {
      Ok((path, loaded, parse_file(path, &files, &ops_cache, i, prelude)?))
    })
    .collect::<Result<Vec<_>, Rc<dyn ProjectError>>>()?;
  // sort by similarity, then longest-first
  entries.sort_unstable_by(|a, b| a.0.cmp(b.0).reverse());
  let files = entries
    .into_iter()
    .map(|(path, loaded, parsed)| ParsedSource {
      loaded,
      parsed,
      path: path.clone(),
    })
    .collect::<Vec<_>>();
  Ok(ProjectTree(files_to_module(Substack::Bottom, files, i)))
}
