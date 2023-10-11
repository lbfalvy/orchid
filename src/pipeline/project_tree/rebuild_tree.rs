use hashbrown::HashMap;
use itertools::Itertools;

use super::build_tree::{build_tree, TreeReport};
use super::import_tree::{import_tree, ImpMod};
use crate::error::ProjectResult;
use crate::pipeline::source_loader::{
  LoadedSourceTable, PreExtra, PreMod, Preparsed,
};
use crate::representations::project::{ProjectExt, ProjectMod};
use crate::sourcefile::FileEntry;
use crate::tree::{ModEntry, ModMember, Module};
use crate::utils::pure_seq::pushed_ref;
use crate::utils::unwrap_or;
use crate::{Interner, ProjectTree, Tok, VName};

pub fn rebuild_file(
  path: Vec<Tok<String>>,
  pre: PreMod,
  imports: ImpMod,
  source: &LoadedSourceTable,
  prelude: &[FileEntry],
) -> ProjectResult<ProjectMod<VName>> {
  let file = match &pre.extra {
    PreExtra::Dir => panic!("Dir should not hand this node off"),
    PreExtra::Submod(_) => panic!("should not have received this"),
    PreExtra::File(f) => f,
  };
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
  let entries = src.entries.clone();
  let TreeReport { entries, rules, imports_from } =
    build_tree(&path, entries, pre, imports, prelude)?;
  let file = Some(path.clone());
  Ok(Module { entries, extra: ProjectExt { file, path, imports_from, rules } })
}

pub fn rebuild_dir(
  path: Vec<Tok<String>>,
  pre: PreMod,
  mut imports: ImpMod,
  source: &LoadedSourceTable,
  prelude: &[FileEntry],
) -> ProjectResult<ProjectMod<VName>> {
  match pre.extra {
    PreExtra::Dir => (),
    PreExtra::File(_) =>
      return rebuild_file(path, pre, imports, source, prelude),
    PreExtra::Submod(_) => panic!("Dirs contain dirs and files"),
  }
  let entries = (pre.entries.into_iter())
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
      let module = rebuild_dir(path, pre, impmod, source, prelude)?;
      Ok((name, ModEntry { exported, member: ModMember::Sub(module) }))
    })
    .collect::<Result<HashMap<_, _>, _>>()?;
  Ok(Module {
    extra: ProjectExt {
      path,
      imports_from: HashMap::new(),
      rules: Vec::new(),
      file: None,
    },
    entries,
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
  rebuild_dir(Vec::new(), preparsed.0, imports, source, prelude)
    .map(ProjectTree)
}
