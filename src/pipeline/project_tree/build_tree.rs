use hashbrown::HashMap;
use itertools::{Either, Itertools};

use super::import_tree::ImpMod;
use crate::ast::{Constant, Rule};
use crate::error::{ConflictingRoles, ProjectError, ProjectResult};
use crate::pipeline::source_loader::{PreItem, PreMod};
use crate::representations::project::{
  ImpReport, ItemKind, ProjectEntry, ProjectExt, ProjectItem,
};
use crate::sourcefile::{
  FileEntry, FileEntryKind, Member, MemberKind, ModuleBlock,
};
use crate::tree::{ModEntry, ModMember, Module};
use crate::utils::get_or::get_or_default;
use crate::utils::pure_seq::pushed_ref;
use crate::{Tok, VName};

#[must_use = "A submodule may not be integrated into the tree"]
pub struct TreeReport {
  pub entries: HashMap<Tok<String>, ProjectEntry<VName>>,
  pub rules: Vec<Rule<VName>>,
  /// Maps imported symbols to the absolute paths of the modules they are
  /// imported from
  pub imports_from: HashMap<Tok<String>, ImpReport<VName>>,
}

pub fn build_tree(
  path: &VName,
  source: Vec<FileEntry>,
  Module { entries, .. }: PreMod,
  imports: ImpMod,
  prelude: &[FileEntry],
) -> ProjectResult<TreeReport> {
  let source =
    source.into_iter().chain(prelude.iter().cloned()).collect::<Vec<_>>();
  let (imports_from, mut submod_imports) = (imports.entries.into_iter())
    .partition_map::<HashMap<_, _>, HashMap<_, _>, _, _, _>(
      |(n, ent)| match ent.member {
        ModMember::Item(it) => Either::Left((n, it)),
        ModMember::Sub(s) => Either::Right((n, s)),
      },
    );
  let mut rule_fragments = Vec::new();
  let mut submodules = HashMap::<_, Vec<_>>::new();
  let mut consts = HashMap::new();
  for FileEntry { kind, locations: _ } in source {
    match kind {
      FileEntryKind::Import(_) => (),
      FileEntryKind::Comment(_) => (),
      FileEntryKind::Export(_) => (),
      FileEntryKind::Member(Member { kind, .. }) => match kind {
        MemberKind::Module(ModuleBlock { body, name }) => {
          get_or_default(&mut submodules, &name).extend(body.into_iter());
        },
        MemberKind::Constant(Constant { name, value }) => {
          consts.insert(name, value /* .prefix(path, &|_| false) */);
        },
        MemberKind::Rule(rule) => rule_fragments.push(rule),
      },
    }
  }
  let rules = rule_fragments;
  let (pre_subs, pre_items) = (entries.into_iter())
    .partition_map::<HashMap<_, _>, HashMap<_, _>, _, _, _>(
      |(k, ModEntry { exported, member })| match member {
        ModMember::Sub(s) => Either::Left((k, (exported, s))),
        ModMember::Item(it) => Either::Right((k, (exported, it))),
      },
    );
  let mut entries = (pre_subs.into_iter())
    .map(|(k, (exported, pre_member))| {
      let impmod = (submod_imports.remove(&k))
        .expect("Imports and preparsed should line up");
      (k, exported, pre_member, impmod)
    })
    .map(|(k, exported, pre, imp)| {
      let source = submodules
        .remove(&k)
        .expect("Submodules should not disappear after reparsing");
      (k, exported, pre, imp, source)
    })
    .map(|(k, exported, pre, imp, source)| {
      let path = pushed_ref(path, k.clone());
      let TreeReport { entries, imports_from, rules } =
        build_tree(&path, source, pre, imp, prelude)?;
      let extra = ProjectExt { path, file: None, imports_from, rules };
      let member = ModMember::Sub(Module { entries, extra });
      Ok((k, ModEntry { exported, member }))
    })
    .chain((pre_items.into_iter()).map(
      |(k, (exported, PreItem { has_value, location }))| {
        let item = match imports_from.get(&k) {
          Some(_) if has_value => {
            // Local value cannot be assigned to imported key
            let const_loc =
              consts.remove(&k).expect("has_value is true").location;
            let err = ConflictingRoles {
              locations: vec![location, const_loc],
              name: pushed_ref(path, k),
            };
            return Err(err.rc());
          },
          None => {
            let k = consts.remove(&k).map_or(ItemKind::None, ItemKind::Const);
            ProjectItem { kind: k }
          },
          Some(report) =>
            ProjectItem { kind: ItemKind::Alias(report.source.clone()) },
        };
        Ok((k, ModEntry { exported, member: ModMember::Item(item) }))
      },
    ))
    .collect::<Result<HashMap<_, _>, _>>()?;
  for (k, from) in imports_from.iter() {
    let (_, ent) = entries.raw_entry_mut().from_key(k).or_insert_with(|| {
      (k.clone(), ModEntry {
        exported: false,
        member: ModMember::Item(ProjectItem {
          kind: ItemKind::Alias(from.source.clone()),
        }),
      })
    });
    debug_assert!(
      matches!(
        ent.member,
        ModMember::Item(ProjectItem { kind: ItemKind::Alias(_), .. })
      ),
      "Should have emerged in the processing of pre_items"
    )
  }
  Ok(TreeReport { entries, rules, imports_from })
}
