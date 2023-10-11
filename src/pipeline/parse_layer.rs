use super::dealias::resolve_aliases;
use super::file_loader::IOResult;
use super::{project_tree, source_loader};
use crate::error::ProjectResult;
use crate::interner::{Interner, Tok};
use crate::parse::{LexerPlugin, LineParser};
use crate::representations::sourcefile::FileEntry;
use crate::representations::VName;
use crate::utils::never;
use crate::ProjectTree;

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
  lexer_plugins: &[&dyn LexerPlugin],
  line_parsers: &[&dyn LineParser],
  i: &Interner,
) -> ProjectResult<ProjectTree<VName>> {
  let sl_ctx =
    source_loader::Context { prelude, i, lexer_plugins, line_parsers };
  let (preparsed, source) =
    source_loader::load_source(targets, sl_ctx, loader, &|path| {
      environment.0.walk_ref(&[], path, false).is_ok()
    })?;
  let tree =
    project_tree::rebuild_tree(&source, preparsed, environment, prelude, i)?;
  let sum = ProjectTree(never::unwrap_always(
    environment.0.clone().overlay(tree.0.clone()),
  ));
  let resolvd =
    resolve_aliases(sum, &|path| tree.0.walk1_ref(&[], path, false).is_ok());
  // Addition among modules favours the left hand side.
  Ok(resolvd)
}
