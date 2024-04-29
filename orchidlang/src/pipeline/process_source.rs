use std::mem;
use std::sync::Arc;

use hashbrown::HashMap;
use intern_all::Tok;
use itertools::Itertools;
use never::Never;

use super::load_project::{Prelude, ProjectContext};
use super::path::absolute_path;
use super::project::{
  ItemKind, ProjItem, ProjRule, ProjXEnt, ProjXMod, ProjectEntry, ProjectMod, SourceModule,
};
use crate::error::{ErrorPosition, ProjectError, ProjectErrorObj, Reporter};
use crate::location::{CodeLocation, CodeOrigin, SourceRange};
use crate::name::{Sym, VName, VPath};
use crate::parse::parsed::{
  Constant, Member, MemberKind, ModuleBlock, Rule, SourceLine, SourceLineKind,
};
use crate::tree::{ModEntry, ModMember, Module, WalkError};
use crate::utils::combine::Combine;
use crate::utils::get_or::get_or_make;
use crate::utils::sequence::Sequence;

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
  pub ext_refs: HashMap<Sym, SourceRange>,
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
  path: Sym,
  lines: Vec<SourceLine>,
  ns_location: SourceRange,
  reporter: &Reporter,
) -> FileReport {
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
        for (name, name_loc) in names {
          let entry = get_or_make(&mut entries, &name, default_entry);
          entry.x.locations.push(CodeLocation::new_src(name_loc, path.clone()));
          if entry.x.exported {
            reporter.report(MultipleExports::new(path.clone(), name.clone(), entry).pack());
          }
          entry.x.exported = true;
          entry.x.comments.extend(comments.iter().flatten().cloned());
        }
      },
      SourceLineKind::Import(imports) => {
        file_comments.append(&mut new_comments);
        for import in imports {
          let nonglob_path = import.nonglob_path();
          let origin = CodeOrigin::Source(range.clone());
          match absolute_path(&path[..], &nonglob_path[..]) {
            Err(e) => reporter.report(e.bundle(&origin)),
            Ok(abs) => {
              if !abs[..].starts_with(&path[..]) {
                external_references.insert(abs.to_sym(), import.range.clone());
              }
              match import.name {
                None =>
                  (glob_imports.x.0).push(GlobImpReport { target: abs, location: import.range }),
                Some(name) => {
                  let entry = get_or_make(&mut entries, &name, default_entry);
                  entry.x.locations.push(CodeLocation::new_src(import.range, path.clone()));
                  if let ModMember::Item(ProjItem { kind: old @ ItemKind::None }) =
                    &mut entry.member
                  {
                    *old = ItemKind::Alias(abs.to_sym())
                  } else {
                    reporter.report(MultipleDefinitions::new(path.clone(), name, entry).pack());
                  }
                },
              }
            },
          }
        }
      },
      SourceLineKind::Member(Member { exported, kind }) => match kind {
        MemberKind::Constant(Constant { name, value }) => {
          let entry = get_or_make(&mut entries, &name, default_entry);
          entry.x.locations.push(CodeLocation::new_src(range, path.clone()));
          if let ModMember::Item(ProjItem { kind: old @ ItemKind::None }) = &mut entry.member {
            *old = ItemKind::Const(value)
          } else {
            reporter.report(MultipleDefinitions::new(path.clone(), name.clone(), entry).pack());
          }
          entry.x.exported |= exported;
          entry.x.comments.append(&mut new_comments);
        },
        MemberKind::Rule(Rule { pattern, prio, template }) => {
          let prule = ProjRule { pattern, prio, template, comments: new_comments };
          new_comments = Vec::new();
          for name in prule.collect_root_names() {
            let entry = get_or_make(&mut entries, &name, default_entry);
            entry.x.locations.push(CodeLocation::new_src(range.clone(), path.clone()));
            if entry.x.exported && exported {
              reporter.report(MultipleExports::new(path.clone(), name.clone(), entry).pack());
            }
            entry.x.exported |= exported;
          }
          rules.push(prule);
        },
        MemberKind::Module(ModuleBlock { name, body }) => {
          let entry = get_or_make(&mut entries, &name, default_entry);
          (entry.x.locations).push(CodeLocation::new_src(range.clone(), path.clone()));
          if !matches!(entry.member, ModMember::Item(ProjItem { kind: ItemKind::None })) {
            reporter.report(MultipleDefinitions::new(path.clone(), name.clone(), entry).pack());
          }
          if entry.x.exported && exported {
            reporter.report(MultipleExports::new(path.clone(), name.clone(), entry).pack());
          }
          let subpath = path.to_vname().suffix([name.clone()]).to_sym();
          let mut report = process_ns(subpath, body, range, reporter);
          entry.x.comments.append(&mut new_comments);
          entry.x.comments.extend(report.comments);
          entry.x.exported |= exported;
          if let ModMember::Sub(module) = &entry.member {
            // This is an error state.
            report.module.entries.extend(module.entries.clone());
          }
          entry.member = ModMember::Sub(report.module);
          // record new external references
          external_references
            .extend(report.ext_refs.into_iter().filter(|(r, _)| !r[..].starts_with(&path[..])));
          // add glob_imports subtree to own tree
          glob_imports
            .entries
            .insert(name, ModEntry { x: (), member: ModMember::Sub(report.glob_imports) });
        },
      },
    }
  }
  FileReport {
    ext_refs: external_references,
    comments: file_comments,
    glob_imports,
    module: Module {
      entries,
      x: ProjXMod { src: Some(SourceModule { range: ns_location, rules }) },
    },
  }
}

fn walk_at_path(e: WalkError, root: &ProjectMod, path: &[Tok<String>]) -> ProjectErrorObj {
  let submod =
    (root.walk_ref(&[], path, |_| true)).expect("Invalid source path in walk error populator");
  let src = submod.x.src.as_ref().expect("Import cannot appear in implied module");
  e.at(&src.range.origin())
}

pub fn resolve_globs_rec(
  // must exist in project_root
  path: VPath,
  globtree: GlobImports,
  preludes: Sequence<&Prelude>,
  project_root: &mut ProjectMod,
  env: &Module<impl Sized, impl Sized, impl Sized>,
  contention: &mut HashMap<(Sym, Sym), Vec<CodeOrigin>>,
  reporter: &Reporter,
) {
  // All glob imports in this module
  let all =
    (globtree.x.0.into_iter()).map(|gir| (gir.target, CodeOrigin::Source(gir.location))).chain(
      preludes
        .iter()
        .filter(|&pre| !path.0.starts_with(&pre.exclude[..]))
        .map(|Prelude { target, owner, .. }| (target.clone(), CodeOrigin::Gen(owner.clone()))),
    );
  if !path[..].is_empty() {
    for (target, imp_loc) in all {
      let pub_keys = match project_root.inner_walk(&path.0, &target[..], |e| e.x.exported) {
        Err(e) => {
          reporter.report(walk_at_path(e, project_root, &path.0));
          continue;
        },
        Ok((ModEntry { member: ModMember::Item(_), .. }, parent)) => {
          use crate::tree::ErrKind::NotModule;
          let options = Sequence::new(|| parent.keys(|e| e.x.exported));
          let e = WalkError::last(&target[..], NotModule, options);
          reporter.report(walk_at_path(e, project_root, &path.0));
          continue;
        },
        // All public keys in this module and, if walkable, the environment.
        Ok((ModEntry { member: ModMember::Sub(module), .. }, _)) =>
          (env.walk_ref(&[], &target[..], |_| true).into_iter())
            .flat_map(|m| m.keys(|_| true))
            .chain(module.keys(|e| e.x.exported))
            .collect_vec(),
      };
      // Reference to the module to be modified
      let mut_mod = path.0.iter().fold(&mut *project_root, |m, k| {
        let entry = m.entries.get_mut(k).expect("this is a source path");
        if let ModMember::Sub(s) = &mut entry.member { s } else { panic!("This is a source path") }
      });
      // Walk errors for the environment are suppressed because leaf-node
      // conflicts will emerge when merging modules, and walking off the tree
      // is valid.
      for key in pub_keys {
        let entry = get_or_make(&mut mut_mod.entries, &key, default_entry);
        entry.x.locations.push(CodeLocation {
          origin: imp_loc.clone(),
          module: path.clone().into_name().expect("Checked above").to_sym(),
        });
        let alias_tgt = target.clone().suffix([key.clone()]).to_sym();
        if let ModMember::Item(ProjItem { kind: kref @ ItemKind::None }) = &mut entry.member {
          *kref = ItemKind::Alias(alias_tgt)
        } else {
          let local_name = path.clone().name_with_prefix(key.clone()).to_sym();
          contention.insert((alias_tgt, local_name), entry.x.origins().collect());
        }
      }
    }
  }
  for (key, entry) in globtree.entries {
    match entry.member {
      ModMember::Item(n) => match n {},
      ModMember::Sub(module) => {
        resolve_globs_rec(
          // Submodules in globtree must correspond to submodules in project
          path.clone().suffix([key]),
          module,
          preludes.clone(),
          project_root,
          env,
          contention,
          reporter,
        )
      },
    }
  }
}

/// Resolve the glob tree separately produced by [process_ns] by looking up the
/// keys of the referenced module and creating an [ItemKind::Alias] for each of
/// them. Supports a prelude table which is applied to each module, and an
/// environment whose keys are combined with those from within the [ProjectMod].
pub fn resolve_globs(
  globtree: GlobImports,
  ctx: &ProjectContext,
  project_root: &mut ProjectMod,
  env: &Module<impl Sized, impl Sized, impl Sized>,
  contentions: &mut HashMap<(Sym, Sym), Vec<CodeOrigin>>,
) {
  let preludes = ctx.preludes.clone();
  resolve_globs_rec(VPath(vec![]), globtree, preludes, project_root, env, contentions, ctx.reporter)
}

struct MultipleExports {
  path: Sym,
  locations: Vec<CodeOrigin>,
}
impl MultipleExports {
  fn new(mpath: Sym, name: Tok<String>, entry: &'_ ProjectEntry) -> Self {
    Self { path: mpath.to_vname().suffix([name]).to_sym(), locations: entry.x.origins().collect() }
  }
}
impl ProjectError for MultipleExports {
  const DESCRIPTION: &'static str = "A symbol was exported in multiple places";
  fn message(&self) -> String { format!("{} exported multiple times", self.path) }
  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> {
    (self.locations.iter()).map(|l| ErrorPosition { origin: l.clone(), message: None })
  }
}

pub(super) struct MultipleDefinitions {
  pub(super) path: Sym,
  pub(super) locations: Vec<CodeOrigin>,
}
impl MultipleDefinitions {
  fn new(mpath: Sym, name: Tok<String>, entry: &'_ ProjectEntry) -> Self {
    Self { path: mpath.to_vname().suffix([name]).to_sym(), locations: entry.x.origins().collect() }
  }
}
impl ProjectError for MultipleDefinitions {
  const DESCRIPTION: &'static str = "Symbol defined twice";
  fn message(&self) -> String { format!("{} refers to multiple conflicting items", self.path) }
  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> {
    (self.locations.iter()).map(|l| ErrorPosition { origin: l.clone(), message: None })
  }
}
