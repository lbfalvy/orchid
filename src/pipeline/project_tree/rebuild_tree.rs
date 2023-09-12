use std::rc::Rc;

use hashbrown::HashMap;
use itertools::Itertools;

use super::build_tree::{build_tree, TreeReport};
use super::import_tree::{import_tree, ImpMod};
use crate::error::ProjectResult;
use crate::pipeline::source_loader::{
  LoadedSourceTable, PreExtra, PreItem, PreMod, Preparsed,
};
use crate::representations::project::{ImpReport, ProjectExt, ProjectMod};
use crate::sourcefile::FileEntry;
use crate::tree::{ModEntry, ModMember, Module};
use crate::utils::never::{always, unwrap_always};
use crate::utils::pushed::pushed_ref;
use crate::utils::unwrap_or;
use crate::{parse, Interner, ProjectTree, Tok, VName};

pub fn rebuild_file(
  path: Vec<Tok<String>>,
  pre: PreMod,
  imports: ImpMod,
  source: &LoadedSourceTable,
  prelude: &[FileEntry],
  i: &Interner,
) -> ProjectResult<ProjectMod<VName>> {
  let file = match &pre.extra {
    PreExtra::Dir => panic!("Dir should not hand this node off"),
    PreExtra::Submod(_) => panic!("should not have received this"),
    PreExtra::File(f) => f,
  };
  let mut ops = Vec::new();
  unwrap_always(imports.search_all((), &mut |_, module, ()| {
    ops.extend(
      (module.entries.iter())
        .filter(|(_, ent)| {
          matches!(ent.member, ModMember::Item(ImpReport { is_op: true, .. }))
        })
        .map(|(name, _)| name.clone()),
    );
    always(())
  }));
  unwrap_always(pre.search_all((), &mut |_, module, ()| {
    ops.extend(
      (module.entries.iter())
        .filter(|(_, ent)| {
          matches!(ent.member, ModMember::Item(PreItem { is_op: true, .. }))
        })
        .map(|(name, _)| name.clone()),
    );
    always(())
  }));
  let ctx = parse::ParsingContext::new(&ops, i, Rc::new(path.clone()));
  let src = source.get(&file.name).unwrap_or_else(|| {
    panic!(
      "{} should have been preparsed already. Preparsed files are {}",
      Interner::extern_all(&file.name).join("/") + ".orc",
      source
        .keys()
        .map(|f| Interner::extern_all(f).join("/") + ".orc")
        .join(", ")
    )
  });
  let entries = parse::parse2(&src.text, ctx)?;
  let TreeReport { entries: items, rules, imports_from } =
    build_tree(&path, entries, pre, imports, prelude)?;
  Ok(Module {
    entries: items,
    extra: ProjectExt { file: Some(path.clone()), path, imports_from, rules },
  })
}

pub fn rebuild_dir(
  path: Vec<Tok<String>>,
  pre: PreMod,
  mut imports: ImpMod,
  source: &LoadedSourceTable,
  prelude: &[FileEntry],
  i: &Interner,
) -> ProjectResult<ProjectMod<VName>> {
  match pre.extra {
    PreExtra::Dir => (),
    PreExtra::File(_) =>
      return rebuild_file(path, pre, imports, source, prelude, i),
    PreExtra::Submod(_) => panic!("Dirs contain dirs and files"),
  }
  let items = (pre.entries.into_iter())
    .map(|(name, entry)| {
      match imports.entries.remove(&name).map(|e| e.member) {
        Some(ModMember::Sub(impmod)) => (name, entry, impmod),
        _ => panic!("Imports must line up with modules"),
      }
    })
    .map(|(name, ModEntry { member, exported }, impmod)| -> ProjectResult<_> {
      let path = pushed_ref(&path, name.clone());
      let pre = unwrap_or!(member => ModMember::Sub;
        panic!("Dirs can only contain submodules")
      );
      Ok((name, ModEntry {
        exported,
        member: ModMember::Sub(rebuild_dir(
          path, pre, impmod, source, prelude, i,
        )?),
      }))
    })
    .collect::<Result<HashMap<_, _>, _>>()?;
  Ok(Module {
    extra: ProjectExt {
      path,
      imports_from: HashMap::new(),
      rules: Vec::new(),
      file: None,
    },
    entries: items,
  })
}

/// Rebuild the entire tree
pub fn rebuild_tree(
  source: &LoadedSourceTable,
  preparsed: Preparsed,
  prev_root: &ProjectTree<VName>,
  prelude: &[FileEntry],
  i: &Interner,
) -> ProjectResult<ProjectTree<VName>> {
  let imports =
    import_tree(Vec::new(), &preparsed.0, &preparsed, prev_root, i)?;
  rebuild_dir(Vec::new(), preparsed.0, imports, source, prelude, i)
    .map(ProjectTree)
}
