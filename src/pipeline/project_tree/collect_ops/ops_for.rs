use std::rc::Rc;

use hashbrown::HashSet;

use super::exported_ops::{ExportedOpsCache, OpsResult};
use crate::interner::{Interner, Tok};
use crate::parse::is_op;
use crate::pipeline::error::ProjectError;
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
      tree_all_ops(m.as_ref(), ops);
    }
  }
}

/// Collect all names imported in this file
pub fn collect_ops_for(
  file: &[Tok<String>],
  loaded: &LoadedSourceTable,
  ops_cache: &ExportedOpsCache,
  i: &Interner,
) -> OpsResult {
  let tree = &loaded[&i.i(file)].preparsed.0;
  let mut ret = HashSet::new();
  tree_all_ops(tree.as_ref(), &mut ret);
  tree.visit_all_imports(&mut |modpath, _module, import| {
    if let Some(n) = import.name {
      ret.insert(n);
    } else {
      let path = import_abs_path(file, modpath, &i.r(import.path)[..], i)
        .expect("This error should have been caught during loading");
      ret.extend(ops_cache.find(&i.i(&path))?.iter().copied());
    }
    Ok::<_, Rc<dyn ProjectError>>(())
  })?;
  ret.drain_filter(|t| !is_op(i.r(*t)));
  Ok(Rc::new(ret))
}
