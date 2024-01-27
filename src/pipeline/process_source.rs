use std::mem;
use std::sync::Arc;

use hashbrown::HashMap;
use intern_all::Tok;
use itertools::Itertools;
use never::Never;

use super::load_solution::Prelude;
use super::path::absolute_path;
use super::project::{
  ItemKind, ProjItem, ProjRule, ProjXEnt, ProjXMod, ProjectEntry, ProjectMod,
  SourceModule,
};
use crate::error::{
  ErrorPosition, ProjectError, ProjectErrorObj, ProjectResult,
};
use crate::location::{CodeLocation, SourceRange};
use crate::name::{Sym, VName, VPath};
use crate::parse::parsed::{
  Constant, Member, MemberKind, ModuleBlock, Rule, SourceLine, SourceLineKind,
};
use crate::tree::{ModEntry, ModMember, Module, WalkError};
use crate::utils::combine::Combine;
use crate::utils::get_or::get_or_make;
use crate::utils::pure_seq::pushed_ref;
use crate::utils::sequence::Sequence;
use crate::utils::unwrap_or::unwrap_or;

// Problem: import normalization
//
// Imports aren't explicitly present in the tree we're currently producing.
// Named imports can be placed in Aliases, but glob imports should
// not be included in the Project Tree. A separate Glob Import Tree
// should be produced, which preferably utilizes the existing [Module]
// tree. Then a postprocessing step can use the GIT to both look up the exports
// and write them back into &mut PT.

/// This tree contains the absolute path of glob imports.

#[derive(Debug, Clone)]
pub(super) struct GlobImpReport {
  pub target: VName,
  pub location: SourceRange,
}

#[derive(Debug, Clone, Default)]
pub(super) struct GlobImpXMod(pub Vec<GlobImpReport>);
impl Combine for GlobImpXMod {
  type Error = Never;
  fn combine(self, other: Self) -> Result<Self, Self::Error> {
    Ok(GlobImpXMod(self.0.into_iter().chain(other.0).collect()))
  }
}

pub(super) type GlobImports = Module<Never, GlobImpXMod, ()>;

pub(super) struct FileReport {
  /// Absolute path of values outside the file
  pub external_references: HashMap<Sym, SourceRange>,
  pub comments: Vec<Arc<String>>,
  pub module: ProjectMod,
  pub glob_imports: GlobImports,
}

fn default_entry() -> ProjectEntry {
  ProjectEntry {
    member: ModMember::Item(ProjItem::default()),
    x: ProjXEnt { comments: vec![], locations: vec![], exported: false },
  }
}

pub(super) fn process_ns(
  path: Arc<VPath>,
  lines: Vec<SourceLine>,
  ns_location: SourceRange,
) -> ProjectResult<FileReport> {
  let mut file_comments = Vec::new();
  let mut new_comments = Vec::new();
  let mut entries = HashMap::new();
  let mut external_references = HashMap::new();
  let mut rules = Vec::new();
  let wrap = Module::wrap([]);
  let mut glob_imports: GlobImports = wrap;
  for SourceLine { kind: line_kind, range } in lines {
    match line_kind {
      SourceLineKind::Comment(comment) => new_comments.push(Arc::new(comment)),
      SourceLineKind::Export(names) => {
        let comments = (names.len() == 1).then(|| mem::take(&mut new_comments));
        for (name, name_location) in names {
          let entry = get_or_make(&mut entries, &name, default_entry);
          let location = CodeLocation::Source(name_location);
          if entry.x.exported {
            let err = MultipleExports {
              path: (*path).clone().as_prefix_of(name).to_sym(),
              locations: pushed_ref(&entry.x.locations, location),
            };
            return Err(err.pack());
          }
          entry.x.exported = true;
          entry.x.comments.extend(comments.iter().flatten().cloned());
          entry.x.locations.push(location);
        }
      },
      SourceLineKind::Import(imports) => {
        file_comments.append(&mut new_comments);
        for import in imports {
          let nonglob_path = import.nonglob_path();
          let location = CodeLocation::Source(range.clone());
          let abs = absolute_path(&path[..], &nonglob_path[..], location)?;
          if !abs[..].starts_with(&path[..]) {
            external_references.insert(abs.to_sym(), import.range.clone());
          }
          match import.name {
            None => (glob_imports.x.0)
              .push(GlobImpReport { target: abs, location: import.range }),
            Some(key) => {
              let entry = get_or_make(&mut entries, &key, default_entry);
              entry.x.locations.push(CodeLocation::Source(import.range));
              match &mut entry.member {
                ModMember::Item(ProjItem { kind: old @ ItemKind::None }) =>
                  *old = ItemKind::Alias(abs.to_sym()),
                _ => {
                  let err = MultipleDefinitions {
                    path: (*path).clone().as_prefix_of(key).to_sym(),
                    locations: entry.x.locations.clone(),
                  };
                  return Err(err.pack());
                },
              }
            },
          }
        }
      },
      SourceLineKind::Member(Member { exported, kind }) => match kind {
        MemberKind::Constant(Constant { name, value }) => {
          let entry = get_or_make(&mut entries, &name, default_entry);
          entry.x.locations.push(CodeLocation::Source(range));
          match &mut entry.member {
            ModMember::Item(ProjItem { kind: old @ ItemKind::None }) =>
              *old = ItemKind::Const(value),
            _ => {
              let err = MultipleDefinitions {
                path: (*path).clone().as_prefix_of(name).to_sym(),
                locations: entry.x.locations.clone(),
              };
              return Err(err.pack());
            },
          }
          entry.x.exported |= exported;
          entry.x.comments.append(&mut new_comments);
        },
        MemberKind::Rule(Rule { pattern, prio, template }) => {
          let prule =
            ProjRule { pattern, prio, template, comments: new_comments };
          new_comments = Vec::new();
          for name in prule.collect_root_names() {
            let entry = get_or_make(&mut entries, &name, default_entry);
            entry.x.locations.push(CodeLocation::Source(range.clone()));
            if entry.x.exported && exported {
              let err = MultipleExports {
                path: (*path).clone().as_prefix_of(name).to_sym(),
                locations: entry.x.locations.clone(),
              };
              return Err(err.pack());
            }
            entry.x.exported |= exported;
          }
          rules.push(prule);
        },
        MemberKind::Module(ModuleBlock { name, body }) => {
          let entry = get_or_make(&mut entries, &name, default_entry);
          entry.x.locations.push(CodeLocation::Source(range.clone()));
          if !matches!(
            entry.member,
            ModMember::Item(ProjItem { kind: ItemKind::None })
          ) {
            let err = MultipleDefinitions {
              path: (*path).clone().as_prefix_of(name).to_sym(),
              locations: entry.x.locations.clone(),
            };
            return Err(err.pack());
          }
          if entry.x.exported && exported {
            let err = MultipleExports {
              path: (*path).clone().as_prefix_of(name).to_sym(),
              locations: entry.x.locations.clone(),
            };
            return Err(err.pack());
          }
          let subpath = Arc::new(VPath(pushed_ref(&path.0, name.clone())));
          let report = process_ns(subpath, body, range)?;
          entry.x.comments.append(&mut new_comments);
          entry.x.comments.extend(report.comments);
          entry.x.exported |= exported;
          entry.member = ModMember::Sub(report.module);
          // record new external references
          external_references.extend(
            (report.external_references.into_iter())
              .filter(|(r, _)| !r[..].starts_with(&path.0)),
          );
          // add glob_imports subtree to own tree
          glob_imports.entries.insert(name, ModEntry {
            x: (),
            member: ModMember::Sub(report.glob_imports),
          });
        },
      },
    }
  }
  Ok(FileReport {
    external_references,
    comments: file_comments,
    glob_imports,
    module: Module {
      entries,
      x: ProjXMod { src: Some(SourceModule { range: ns_location, rules }) },
    },
  })
}

fn walk_at_path(
  e: WalkError,
  root: &ProjectMod,
  path: &[Tok<String>],
) -> ProjectErrorObj {
  let submod = (root.walk_ref(&[], path, |_| true))
    .expect("Invalid source path in walk error populator");
  let src =
    submod.x.src.as_ref().expect("Import cannot appear in implied module");
  e.at(&CodeLocation::Source(src.range.clone()))
}

/// Resolve the glob tree separately produced by [process_ns] by looking up the
/// keys of the referenced module and creating an [ItemKind::Alias] for each of
/// them. Supports a prelude table which is applied to each module, and an
/// environment whose keys are combined with those from within the [ProjectMod].
pub fn resolve_globs(
  globtree_prefix: VPath,
  globtree: GlobImports,
  preludes: Sequence<&Prelude>,
  project_root: &mut ProjectMod,
  env: &Module<impl Sized, impl Sized, impl Sized>,
  contention: &mut HashMap<(Sym, Sym), Vec<CodeLocation>>,
) -> ProjectResult<()> {
  // All glob imports in this module
  let all = (globtree.x.0.into_iter())
    .map(|gir| (gir.target, CodeLocation::Source(gir.location)))
    .chain(
      preludes
        .iter()
        .filter(|&pre| !globtree_prefix.0.starts_with(&pre.exclude[..]))
        .map(|Prelude { target, owner, .. }| {
          (target.clone(), CodeLocation::Gen(owner.clone()))
        }),
    );
  for (target, location) in all {
    let (tgt, parent) = project_root
      .inner_walk(&globtree_prefix.0, &target[..], |e| e.x.exported)
      .map_err(|e| walk_at_path(e, project_root, &globtree_prefix.0))?;
    match &tgt.member {
      ModMember::Item(_) => {
        use crate::tree::ErrKind::NotModule;
        let options = Sequence::new(|| parent.keys(|e| e.x.exported));
        let e = WalkError::last(&target[..], NotModule, options);
        return Err(walk_at_path(e, project_root, &globtree_prefix.0));
      },
      ModMember::Sub(module) => {
        // All public keys in this module and, if walkable, the environment.
        let pub_keys = (env.walk_ref(&[], &target[..], |_| true).into_iter())
          .flat_map(|m| m.keys(|_| true))
          .chain(module.keys(|e| e.x.exported))
          .collect_vec();
        // Reference to the module to be modified
        let mut_mod =
          globtree_prefix.0.iter().fold(&mut *project_root, |m, k| {
            let entry = m.entries.get_mut(k).expect("this is a source path");
            unwrap_or!(&mut entry.member => ModMember::Sub; {
              panic!("This is also a source path")
            })
          });
        // Walk errors for the environment are suppressed because leaf-node
        // conflicts will emerge when merging modules, and walking off the tree
        // is valid.
        for key in pub_keys {
          let entry = get_or_make(&mut mut_mod.entries, &key, default_entry);
          entry.x.locations.push(location.clone());
          let alias_tgt = target.clone().suffix([key.clone()]).to_sym();
          match &mut entry.member {
            ModMember::Item(ProjItem { kind: kref @ ItemKind::None }) =>
              *kref = ItemKind::Alias(alias_tgt),
            ModMember::Item(ProjItem { kind: ItemKind::Alias(prev_alias) }) =>
              if prev_alias != &alias_tgt {
                let local_name =
                  globtree_prefix.clone().as_prefix_of(key.clone()).to_sym();
                let locs = pushed_ref(&entry.x.locations, location.clone());
                contention.insert((alias_tgt, local_name), locs);
              },
            _ => {
              let err = MultipleDefinitions {
                locations: entry.x.locations.clone(),
                path: globtree_prefix.as_prefix_of(key).to_sym(),
              };
              return Err(err.pack());
            },
          }
        }
      },
    }
  }
  for (key, entry) in globtree.entries {
    match entry.member {
      ModMember::Item(n) => match n {},
      ModMember::Sub(module) => {
        let subpath = VPath(pushed_ref(&globtree_prefix.0, key));
        resolve_globs(
          subpath,
          module,
          preludes.clone(),
          project_root,
          env,
          contention,
        )?;
      },
    }
  }
  Ok(())
}

struct MultipleExports {
  path: Sym,
  locations: Vec<CodeLocation>,
}
impl ProjectError for MultipleExports {
  const DESCRIPTION: &'static str = "A symbol was exported in multiple places";
  fn message(&self) -> String {
    format!("{} exported multiple times", self.path)
  }
  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> {
    (self.locations.iter())
      .map(|l| ErrorPosition { location: l.clone(), message: None })
  }
}

pub(super) struct MultipleDefinitions {
  pub(super) path: Sym,
  pub(super) locations: Vec<CodeLocation>,
}
impl ProjectError for MultipleDefinitions {
  const DESCRIPTION: &'static str = "Symbol defined twice";
  fn message(&self) -> String {
    format!("{} refers to multiple conflicting items", self.path)
  }
  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> {
    (self.locations.iter())
      .map(|l| ErrorPosition { location: l.clone(), message: None })
  }
}
