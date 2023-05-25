use std::rc::Rc;

use super::error::ProjectError;
use super::file_loader::IOResult;
use super::{import_resolution, project_tree, source_loader, ProjectTree};
use crate::interner::{Interner, Sym, Tok};
use crate::representations::sourcefile::FileEntry;

/// Using an IO callback, produce a project tree that includes the given
/// target symbols or files if they're defined.
///
/// The environment accessible to the loaded source can be specified with
/// a pre-existing tree which will be merged with the loaded data, and a
/// prelude which will be prepended to each individual file. Since the
/// prelude gets compiled with each file, normally it should be a glob
/// import pointing to a module in the environment.
pub fn parse_layer(
  targets: &[Sym],
  loader: &impl Fn(Sym) -> IOResult,
  environment: &ProjectTree,
  prelude: &[FileEntry],
  i: &Interner,
) -> Result<ProjectTree, Rc<dyn ProjectError>> {
  // A path is injected if it is walkable in the injected tree
  let injected_as = |path: &[Tok<String>]| {
    let (item, modpath) = path.split_last()?;
    let module = environment.0.walk(modpath, false).ok()?;
    let inj = module.extra.exports.get(item).copied()?;
    Some(inj)
  };
  let injected_names = |path: Tok<Vec<Tok<String>>>| {
    let module = environment.0.walk(&i.r(path)[..], false).ok()?;
    Some(Rc::new(module.extra.exports.keys().copied().collect()))
  };
  let source =
    source_loader::load_source(targets, prelude, i, loader, &|path| {
      injected_as(path).is_some()
    })?;
  let tree = project_tree::build_tree(source, i, prelude, &injected_names)?;
  let sum = ProjectTree(Rc::new(
    environment.0.as_ref().clone() + tree.0.as_ref().clone(),
  ));
  let resolvd = import_resolution::resolve_imports(sum, i, &injected_as)?;
  // Addition among modules favours the left hand side.
  Ok(resolvd)
}
