use hashbrown::HashMap;
use std::hash::Hash;
use std::rc::Rc;

use crate::ast::Constant;
use crate::pipeline::error::{ProjectError, ParseErrorWithPath, VisibilityMismatch};
use crate::representations::sourcefile::{normalize_namespaces, Member};
use crate::representations::tree::{ModEntry, ModMember};
use crate::interner::Interner;
use crate::parse::{self, ParsingContext};
use crate::representations::{sourcefile::{FileEntry, imports}, tree::Module};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preparsed(pub Rc<Module<(), ()>>);

/// Add an internal flat name if it does not exist yet
fn add_intern<K: Eq + Hash>(
  map: &mut HashMap<K, ModEntry<(), ()>>, k: K
) {
  let _ = map.try_insert(k, ModEntry {
    exported: false,
    member: ModMember::Item(()),
  });
}

/// Add an exported flat name or export any existing entry
fn add_export<K: Eq + Hash>(
  map: &mut HashMap<K, ModEntry<(), ()>>, k: K
) {
  if let Some(entry) = map.get_mut(&k) {
    entry.exported = true
  } else {
    map.insert(k, ModEntry {
      exported: true,
      member: ModMember::Item(()),
    });
  }
}

/// Convert source lines into a module
fn to_module(
  src: &[FileEntry],
  prelude: &[FileEntry],
  i: &Interner
) -> Rc<Module<(), ()>> {
  let all_src = || src.iter().chain(prelude.iter());
  let imports = imports(all_src()).cloned().collect::<Vec<_>>();
  let mut items = all_src().filter_map(|ent| match ent {
    FileEntry::Internal(Member::Namespace(name, data)) => {
      let member = ModMember::Sub(to_module(data, prelude, i));
      let entry = ModEntry{ exported: false, member };
      Some((*name, entry))
    }
    FileEntry::Exported(Member::Namespace(name, data)) => {
      let member = ModMember::Sub(to_module(data, prelude, i));
      let entry = ModEntry{ exported: true, member };
      Some((*name, entry))
    }
    _ => None
  }).collect::<HashMap<_, _>>();
  for file_entry in all_src() { match file_entry {
    FileEntry::Comment(_) | FileEntry::Import(_)
    | FileEntry::Internal(Member::Namespace(..))
    | FileEntry::Exported(Member::Namespace(..)) => (),
    FileEntry::Export(tokv) => for tok in tokv {
      add_export(&mut items, *tok)
    }
    FileEntry::Internal(Member::Constant(Constant{ name, .. }))
      => add_intern(&mut items, *name),
    FileEntry::Exported(Member::Constant(Constant{ name, .. }))
      => add_export(&mut items, *name),
    FileEntry::Internal(Member::Rule(rule)) => {
      let names = rule.collect_single_names(i);
      for name in names {
        add_intern(&mut items, name)
      }
    }
    FileEntry::Exported(Member::Rule(rule)) => {
      let names = rule.collect_single_names(i);
      for name in names {
        add_export(&mut items, name)
      }
    }
  }}
  Rc::new(Module { imports, items, extra: () })
}

/// Preparse the module. At this stage, only the imports and
/// names defined by the module can be parsed
pub fn preparse(
  file: Vec<String>,
  source: &str,
  prelude: &[FileEntry],
  i: &Interner,
) -> Result<Preparsed, Rc<dyn ProjectError>> {
  // Parse with no operators
  let ctx = ParsingContext::<&str>::new(&[], i, Rc::new(file.clone()));
  let entries = parse::parse(source, ctx)
    .map_err(|error| ParseErrorWithPath{
      full_source: source.to_string(),
      error,
      path: file.clone()
    }.rc())?;
  let normalized = normalize_namespaces(Box::new(entries.into_iter()), i)
    .map_err(|ns| VisibilityMismatch{
      namespace: ns.into_iter().map(|t| i.r(t)).cloned().collect(),
      file: Rc::new(file.clone())
    }.rc())?;
  Ok(Preparsed(to_module(&normalized, prelude, i)))
}