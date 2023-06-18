use std::rc::Rc;

use super::error::ProjectError;
use super::file_loader::IOResult;
use super::{import_resolution, project_tree, source_loader, ProjectTree};
use crate::interner::{Interner, Tok};
use crate::representations::sourcefile::FileEntry;
use crate::representations::VName;

/// Using an IO callback, produce a project tree that includes the given
/// target symbols or files if they're defined.
///
/// The environment accessible to the loaded source can be specified with
/// a pre-existing tree which will be merged with the loaded data, and a
/// prelude which will be prepended to each individual file. Since the
/// prelude gets compiled with each file, normally it should be a glob
/// import pointing to a module in the environment.
pub fn parse_layer<'a>(
  targets: impl Iterator<Item = &'a [Tok<String>]>,
  loader: &impl Fn(&[Tok<String>]) -> IOResult,
  environment: &'a ProjectTree<VName>,
  prelude: &[FileEntry],
  i: &Interner,
) -> Result<ProjectTree<VName>, Rc<dyn ProjectError>> {
  // A path is injected if it is walkable in the injected tree
  let injected_as = |path: &[Tok<String>]| {
    let (item, modpath) = path.split_last()?;
    let module = environment.0.walk_ref(modpath, false).ok()?;
    module.extra.exports.get(item).cloned()
  };
  let injected_names = |path: Tok<Vec<Tok<String>>>| {
    let module = environment.0.walk_ref(&i.r(path)[..], false).ok()?;
    Some(Rc::new(module.extra.exports.keys().copied().collect()))
  };
  let source =
    source_loader::load_source(targets, prelude, i, loader, &|path| {
      environment.0.walk_ref(path, false).is_ok()
    })?;
  let tree = project_tree::build_tree(source, i, prelude, &injected_names)?;
  let sum = ProjectTree(environment.0.clone().overlay(tree.0.clone()));
  let resolvd =
    import_resolution::resolve_imports(sum, i, &injected_as, &|path| {
      tree.0.walk_ref(path, false).is_ok()
    })?;
  // Addition among modules favours the left hand side.
  Ok(resolvd)
}
