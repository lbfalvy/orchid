use std::rc::Rc;

use hashbrown::HashMap;
use itertools::Itertools;

use super::types::{PreFileExt, PreItem, PreSubExt};
use super::{PreExtra, Preparsed};
use crate::ast::{Clause, Constant, Expr};
use crate::error::{
  ConflictingRoles, ProjectError, ProjectResult, VisibilityMismatch,
};
use crate::interner::Interner;
use crate::parse::{self, ParsingContext};
use crate::representations::sourcefile::{FileEntry, MemberKind};
use crate::representations::tree::{ModEntry, ModMember, Module};
use crate::sourcefile::{FileEntryKind, Import, Member, ModuleBlock};
use crate::utils::pushed::pushed;
use crate::utils::{get_or_default, get_or_make};
use crate::{Location, Tok, VName};

struct FileReport {
  entries: HashMap<Tok<String>, ModEntry<PreItem, PreExtra>>,
  patterns: Vec<Vec<Expr<VName>>>,
  imports: Vec<Import>,
}

/// Convert source lines into a module
fn to_module(
  file: &[Tok<String>],
  path: VName,
  src: Vec<FileEntry>,
  prelude: &[FileEntry],
) -> ProjectResult<FileReport> {
  let mut imports = Vec::new();
  let mut patterns = Vec::new();
  let mut items = HashMap::<Tok<String>, (bool, PreItem)>::new();
  let mut to_export = HashMap::<Tok<String>, Vec<Location>>::new();
  let mut submods =
    HashMap::<Tok<String>, (bool, Vec<Location>, Vec<FileEntry>)>::new();
  let entries = prelude.iter().cloned().chain(src.into_iter());
  for FileEntry { kind, locations } in entries {
    match kind {
      FileEntryKind::Import(imp) => imports.extend(imp.into_iter()),
      FileEntryKind::Export(names) =>
        for (t, l) in names {
          get_or_default(&mut to_export, &t).push(l)
        },
      FileEntryKind::Member(Member { exported, kind }) => match kind {
        MemberKind::Constant(Constant { name, .. }) => {
          let (prev_exported, it) = get_or_default(&mut items, &name);
          if it.has_value {
            let err = ConflictingRoles { name: pushed(path, name), locations };
            return Err(err.rc());
          }
          if let Some(loc) = locations.get(0) {
            it.location = it.location.clone().or(loc.clone())
          };
          it.has_value = true;
          *prev_exported |= exported;
        },
        MemberKind::Module(ModuleBlock { name, body }) => {
          if let Some((prev_exported, locv, entv)) = submods.get_mut(&name) {
            if *prev_exported != exported {
              let mut namespace = path;
              namespace.push(name.clone());
              let err = VisibilityMismatch { namespace, file: file.to_vec() };
              return Err(err.rc());
            }
            locv.extend(locations.into_iter());
            entv.extend(body.into_iter())
          } else {
            submods.insert(name.clone(), (exported, locations, body.clone()));
          }
        },
        MemberKind::Operators(ops) =>
          for op in ops {
            let (prev_exported, it) = get_or_default(&mut items, &op);
            if let Some(loc) = locations.get(0) {
              it.location = it.location.clone().or(loc.clone())
            }
            *prev_exported |= exported;
            it.is_op = true;
          },
        MemberKind::Rule(r) => {
          patterns.push(r.pattern.clone());
          if exported {
            for ex in r.pattern {
              ex.search_all(&mut |ex| {
                if let Clause::Name(vname) = &ex.value {
                  if let Ok(name) = vname.iter().exactly_one() {
                    get_or_default(&mut to_export, name)
                      .push(ex.location.clone());
                  }
                }
                None::<()>
              });
            }
          }
        },
      },
      _ => (),
    }
  }
  let mut entries = HashMap::with_capacity(items.len() + submods.len());
  entries.extend(items.into_iter().map(|(name, (exported, it))| {
    (name, ModEntry { member: ModMember::Item(it), exported })
  }));
  for (subname, (exported, locations, body)) in submods {
    let mut name = path.clone();
    entries
      .try_insert(subname.clone(), ModEntry {
        member: ModMember::Sub({
          name.push(subname);
          let FileReport { imports, entries: items, patterns } =
            to_module(file, name.clone(), body, prelude)?;
          Module {
            entries: items,
            extra: PreExtra::Submod(PreSubExt { imports, patterns }),
          }
        }),
        exported,
      })
      .map_err(|_| ConflictingRoles { locations, name }.rc())?;
  }
  for (item, locations) in to_export {
    get_or_make(&mut entries, &item, || ModEntry {
      member: ModMember::Item(PreItem {
        is_op: false,
        has_value: false,
        location: locations[0].clone(),
      }),
      exported: true,
    })
    .exported = true
  }
  Ok(FileReport { entries, imports, patterns })
}

/// Preparse the module. At this stage, only the imports and
/// names defined by the module can be parsed
pub fn preparse(
  file: VName,
  source: &str,
  prelude: &[FileEntry],
  i: &Interner,
) -> ProjectResult<Preparsed> {
  // Parse with no operators
  let ctx = ParsingContext::new(&[], i, Rc::new(file.clone()));
  let entries = parse::parse2(source, ctx)?;
  let FileReport { entries, imports, patterns } =
    to_module(&file, file.clone(), entries, prelude)?;
  let mut module = Module {
    entries,
    extra: PreExtra::File(PreFileExt {
      details: PreSubExt { patterns, imports },
      name: file.clone(),
    }),
  };
  for name in file.iter().rev() {
    module = Module {
      extra: PreExtra::Dir,
      entries: HashMap::from([(name.clone(), ModEntry {
        exported: true,
        member: ModMember::Sub(module),
      })]),
    };
  }
  Ok(Preparsed(module))
}
