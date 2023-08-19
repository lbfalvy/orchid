use hashbrown::HashMap;
use itertools::Itertools;

use super::collect_ops;
use super::collect_ops::InjectedOperatorsFn;
use super::parse_file::parse_file;
use crate::ast::{Clause, Constant, Expr};
use crate::error::{ProjectError, ProjectResult, TooManySupers};
use crate::interner::{Interner, Tok};
use crate::pipeline::source_loader::{LoadedSource, LoadedSourceTable};
use crate::representations::project::{ProjectExt, ProjectTree};
use crate::representations::sourcefile::{absolute_path, FileEntry, Member};
use crate::representations::tree::{ModEntry, ModMember, Module};
use crate::representations::{NameLike, VName};
use crate::tree::{WalkError, WalkErrorKind};
use crate::utils::iter::{box_empty, box_once};
use crate::utils::{pushed, unwrap_or, Substack};

#[derive(Debug)]
struct ParsedSource<'a> {
  path: Vec<Tok<String>>,
  loaded: &'a LoadedSource,
  parsed: Vec<FileEntry>,
}

/// Split a path into file- and subpath in knowledge
///
/// # Errors
///
/// if the path is invalid
#[allow(clippy::type_complexity)] // bit too sensitive here IMO
pub fn split_path<'a>(
  path: &'a [Tok<String>],
  proj: &'a ProjectTree<impl NameLike>,
) -> Result<(&'a [Tok<String>], &'a [Tok<String>]), WalkError> {
  let (end, body) = unwrap_or!(path.split_last(); {
    return Ok((&[], &[]))
  });
  let mut module = (proj.0.walk_ref(body, false))?;
  let entry = (module.items.get(end))
    .ok_or(WalkError { pos: path.len() - 1, kind: WalkErrorKind::Missing })?;
  if let ModMember::Sub(m) = &entry.member {
    module = m;
  }
  let file =
    module.extra.file.as_ref().map(|s| &path[..s.len()]).unwrap_or(path);
  let subpath = &path[file.len()..];
  Ok((file, subpath))
}

/// Convert normalized, prefixed source into a module
///
/// # Panics
///
/// - if there are imports with too many "super" prefixes (this is normally
///   detected in preparsing)
/// - if the preparsed tree is missing a module that exists in the source
fn source_to_module(
  // level
  path: Substack<Tok<String>>,
  preparsed: &Module<impl Clone, impl Clone>,
  // data
  data: Vec<FileEntry>,
  // context
  i: &Interner,
  filepath_len: usize,
) -> ProjectResult<Module<Expr<VName>, ProjectExt<VName>>> {
  let path_v = path.iter().rev_vec_clone();
  let imports = (data.iter())
    .filter_map(|ent| {
      if let FileEntry::Import(impv) = ent { Some(impv.iter()) } else { None }
    })
    .flatten()
    .cloned()
    .collect::<Vec<_>>();
  let imports_from = (imports.iter())
    .map(|imp| -> ProjectResult<_> {
      let mut imp_path_v = imp.path.clone();
      imp_path_v
        .push(imp.name.clone().expect("glob imports had just been resolved"));
      let mut abs_path = absolute_path(&path_v, &imp_path_v, i)
        .expect("should have failed in preparsing");
      let name = abs_path.pop().ok_or_else(|| {
        TooManySupers {
          offender_file: path_v[..filepath_len].to_vec(),
          offender_mod: path_v[filepath_len..].to_vec(),
          path: imp_path_v,
        }
        .rc()
      })?;
      Ok((name, abs_path))
    })
    .collect::<Result<HashMap<_, _>, _>>()?;
  let exports = (data.iter())
    .flat_map(|ent| {
      let mk_ent = |name: Tok<String>| (name.clone(), pushed(&path_v, name));
      match ent {
        FileEntry::Export(names) => Box::new(names.iter().cloned().map(mk_ent)),
        FileEntry::Exported(mem) => match mem {
          Member::Constant(constant) => box_once(mk_ent(constant.name.clone())),
          Member::Module(ns) => box_once(mk_ent(ns.name.clone())),
          Member::Rule(rule) => {
            let mut names = Vec::new();
            for e in rule.pattern.iter() {
              e.search_all(&mut |e| {
                if let Clause::Name(n) = &e.value {
                  if let Some([name]) = n.strip_prefix(&path_v[..]) {
                    names.push((name.clone(), n.clone()))
                  }
                }
                None::<()>
              });
            }
            Box::new(names.into_iter())
          },
        },
        _ => box_empty(),
      }
    })
    .collect::<HashMap<_, _>>();
  let rules = (data.iter())
    .filter_map(|ent| match ent {
      FileEntry::Exported(Member::Rule(rule)) => Some(rule),
      FileEntry::Internal(Member::Rule(rule)) => Some(rule),
      _ => None,
    })
    .cloned()
    .collect::<Vec<_>>();
  let items = (data.into_iter())
    .filter_map(|ent| {
      let member_to_item = |exported, member| match member {
        Member::Module(ns) => {
          let new_prep = unwrap_or!(
            &preparsed.items[&ns.name].member => ModMember::Sub;
            panic!("Preparsed should include entries for all submodules")
          );
          let module = match source_to_module(
            path.push(ns.name.clone()),
            new_prep,
            ns.body,
            i,
            filepath_len,
          ) {
            Err(e) => return Some(Err(e)),
            Ok(t) => t,
          };
          let member = ModMember::Sub(module);
          Some(Ok((ns.name.clone(), ModEntry { exported, member })))
        },
        Member::Constant(Constant { name, value }) => {
          let member = ModMember::Item(value);
          Some(Ok((name, ModEntry { exported, member })))
        },
        _ => None,
      };
      match ent {
        FileEntry::Exported(member) => member_to_item(true, member),
        FileEntry::Internal(member) => member_to_item(false, member),
        _ => None,
      }
    })
    .collect::<Result<HashMap<_, _>, _>>()?;
  Ok(Module {
    imports,
    items,
    extra: ProjectExt {
      imports_from,
      exports,
      rules,
      file: Some(path_v[..filepath_len].to_vec()),
    },
  })
}

fn files_to_module(
  path: Substack<Tok<String>>,
  files: Vec<ParsedSource>,
  i: &Interner,
) -> ProjectResult<Module<Expr<VName>, ProjectExt<VName>>> {
  let lvl = path.len();
  debug_assert!(
    files.iter().map(|f| f.path.len()).max().unwrap() >= lvl,
    "path is longer than any of the considered file paths"
  );
  let path_v = path.iter().rev_vec_clone();
  if files.len() == 1 && files[0].path.len() == lvl {
    return source_to_module(
      path.clone(),
      &files[0].loaded.preparsed.0,
      files[0].parsed.clone(),
      i,
      path.len(),
    );
  }
  let items = (files.into_iter())
    .group_by(|f| f.path[lvl].clone())
    .into_iter()
    .map(|(namespace, files)| -> ProjectResult<_> {
      let subpath = path.push(namespace.clone());
      let files_v = files.collect::<Vec<_>>();
      let module = files_to_module(subpath, files_v, i)?;
      let member = ModMember::Sub(module);
      Ok((namespace, ModEntry { exported: true, member }))
    })
    .collect::<Result<HashMap<_, _>, _>>()?;
  let exports: HashMap<_, _> = (items.keys())
    .map(|name| (name.clone(), pushed(&path_v, name.clone())))
    .collect();
  Ok(Module {
    items,
    imports: vec![],
    extra: ProjectExt {
      exports,
      imports_from: HashMap::new(),
      rules: vec![],
      file: None,
    },
  })
}

pub fn build_tree(
  files: LoadedSourceTable,
  i: &Interner,
  prelude: &[FileEntry],
  injected: &impl InjectedOperatorsFn,
) -> ProjectResult<ProjectTree<VName>> {
  assert!(!files.is_empty(), "A tree requires at least one module");
  let ops_cache = collect_ops::mk_cache(&files, injected);
  let mut entries = files
    .iter()
    .map(|(path, loaded)| {
      Ok((path, loaded, parse_file(path, &files, &ops_cache, i, prelude)?))
    })
    .collect::<ProjectResult<Vec<_>>>()?;
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
  Ok(ProjectTree(files_to_module(Substack::Bottom, files, i)?))
}
