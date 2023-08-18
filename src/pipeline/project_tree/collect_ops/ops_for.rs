use std::rc::Rc;

use hashbrown::HashSet;

use super::exported_ops::{ExportedOpsCache, OpsResult};
use crate::error::ProjectResult;
use crate::interner::{Interner, Tok};
use crate::parse::is_op;
use crate::pipeline::import_abs_path::import_abs_path;
use crate::pipeline::source_loader::LoadedSourceTable;
use crate::representations::tree::{ModMember, Module};

/// Collect all operators and names, exported or local, defined in this
/// tree.
fn tree_all_ops(
  module: &Module<impl Clone, impl Clone>,
  ops: &mut HashSet<Tok<String>>,
) {
  ops.extend(module.items.keys().copied());
  for ent in module.items.values() {
    if let ModMember::Sub(m) = &ent.member {
      tree_all_ops(m, ops);
    }
  }
}

/// Collect all names visible in this file
///
/// # Panics
///
/// if any import contains too many Super calls. This should be caught during
/// preparsing
pub fn collect_ops_for(
  file: &[Tok<String>],
  loaded: &LoadedSourceTable,
  ops_cache: &ExportedOpsCache,
  i: &Interner,
) -> OpsResult {
  let tree = &loaded[file].preparsed.0;
  let mut ret = HashSet::new();
  tree_all_ops(tree, &mut ret);
  tree.visit_all_imports(&mut |modpath, _m, import| -> ProjectResult<()> {
    if let Some(n) = import.name {
      ret.insert(n);
    } else {
      let path = i.expect(
        import_abs_path(file, modpath, &import.path, i),
        "This error should have been caught during loading",
      );
      ret.extend(ops_cache.find(&i.i(&path))?.iter().copied());
    }
    Ok(())
  })?;
  Ok(Rc::new(ret.into_iter().filter(|t| is_op(i.r(*t))).collect()))
}
