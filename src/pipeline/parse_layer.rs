use std::rc::Rc;

use crate::representations::sourcefile::FileEntry;
use crate::interner::{Token, Interner};

use super::{project_tree, import_resolution};
use super::source_loader;
use super::file_loader::IOResult;
use super::error::ProjectError;
use super::ProjectTree;

/// Using an IO callback, produce a project tree that includes the given
/// target symbols or files if they're defined.
/// 
/// The environment accessible to the loaded source can be specified with
/// a pre-existing tree which will be merged with the loaded data, and a
/// prelude which will be prepended to each individual file. Since the
/// prelude gets compiled with each file, normally it should be a glob
/// import pointing to a module in the environment.
pub fn parse_layer<'a>(
  targets: &[Token<Vec<Token<String>>>],
  loader: &impl Fn(Token<Vec<Token<String>>>) -> IOResult,
  environment: &'a ProjectTree,
  prelude: &[FileEntry],
  i: &Interner,
) -> Result<ProjectTree, Rc<dyn ProjectError>> {
  // A path is injected if it is walkable in the injected tree
  let injected_as = |path: &[Token<String>]| {
    let (item, modpath) = path.split_last()?;
    let module = environment.0.walk(modpath, false).ok()?;
    let inj = module.extra.exports.get(item).copied()?;
    Some(inj)
  };
  let injected_names = |path: Token<Vec<Token<String>>>| {
    let pathv = &i.r(path)[..];
    let module = environment.0.walk(&pathv, false).ok()?;
    Some(Rc::new(
      module.extra.exports.keys().copied().collect()
    ))
  };
  let source = source_loader::load_source(
    targets, prelude, i, loader, &|path| injected_as(path).is_some()
  )?;
  let tree = project_tree::build_tree(source, i, prelude, &injected_names)?;
  let sum = ProjectTree(Rc::new(
    environment.0.as_ref().clone()
    + tree.0.as_ref().clone()
  ));
  let resolvd = import_resolution::resolve_imports(sum, i, &injected_as)?;
  // Addition among modules favours the left hand side.
  Ok(resolvd)
}