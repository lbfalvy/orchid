use std::sync::Arc;

use hashbrown::HashMap;

use super::loaded_source::{LoadedSource, LoadedSourceTable};
use super::preparse::preparse;
use super::{PreExtra, Preparsed};
use crate::error::{
  NoTargets, ProjectError, ProjectResult, UnexpectedDirectory,
};
use crate::interner::{Interner, Tok};
use crate::parse::{self, LexerPlugin, LineParser, ParsingContext};
use crate::pipeline::file_loader::{IOResult, Loaded};
use crate::pipeline::import_abs_path::import_abs_path;
use crate::representations::sourcefile::FileEntry;
use crate::tree::Module;
use crate::utils::pure_seq::pushed_ref;
use crate::utils::{split_max_prefix, unwrap_or};
use crate::Location;

#[derive(Clone, Copy)]
pub struct Context<'a> {
  pub prelude: &'a [FileEntry],
  pub i: &'a Interner,
  pub lexer_plugins: &'a [&'a dyn LexerPlugin],
  pub line_parsers: &'a [&'a dyn LineParser],
}

/// Load the source at the given path or all within if it's a collection,
/// and all sources imported from these.
fn load_abs_path_rec(
  referrer: &[Tok<String>],
  abs_path: &[Tok<String>],
  mut all: Preparsed,
  source: &mut LoadedSourceTable,
  get_source: &impl Fn(&[Tok<String>], &[Tok<String>]) -> IOResult,
  is_injected_module: &impl Fn(&[Tok<String>]) -> bool,
  ctx @ Context { i, lexer_plugins, line_parsers, prelude }: Context,
) -> ProjectResult<Preparsed> {
  // # Termination
  //
  // Every recursion of this function either
  // - adds one of the files in the source directory to `visited` or
  // - recursively traverses a directory tree
  // therefore eventually the function exits, assuming that the directory tree
  // contains no cycles.

  // try splitting the path to file, swallowing any IO errors
  let name_split = split_max_prefix(abs_path, &|p| {
    get_source(p, referrer).map(|l| l.is_code()).unwrap_or(false)
  });
  if let Some((filename, _)) = name_split {
    // Termination: exit if entry already visited
    if source.contains_key(filename) {
      return Ok(all);
    }
    // if the filename is valid, load, preparse and record this file
    let text = unwrap_or!(get_source(filename, referrer)? => Loaded::Code; {
      return Err(UnexpectedDirectory { path: filename.to_vec() }.rc())
    });
    let entries = parse::parse_file(ParsingContext::new(
      i,
      Arc::new(filename.to_vec()),
      text,
      lexer_plugins,
      line_parsers,
    ))?;
    let preparsed = preparse(filename.to_vec(), entries.clone(), prelude)?;
    source.insert(filename.to_vec(), LoadedSource { entries });
    // recurse on all imported modules
    // will be taken and returned by the closure. None iff an error is thrown
    all = preparsed.0.search_all(all, &mut |modpath,
                                             module,
                                             mut all|
     -> ProjectResult<_> {
      let details = unwrap_or!(module.extra.details(); return Ok(all));
      let referrer = modpath.iter().rev_vec_clone();
      for import in &details.imports {
        let origin = &Location::Unknown;
        let abs_pathv = import_abs_path(
          filename,
          modpath.clone(),
          &import.nonglob_path(),
          i,
          origin,
        )?;
        if abs_path.starts_with(&abs_pathv) {
          continue;
        }
        // recurse on imported module
        all = load_abs_path_rec(
          &referrer,
          &abs_pathv,
          all,
          source,
          get_source,
          is_injected_module,
          ctx,
        )?;
      }
      Ok(all)
    })?;
    // Combine the trees
    all.0.overlay(preparsed.0).map(Preparsed)
  } else {
    // If the path is not within a file, load it as directory
    let coll = match get_source(abs_path, referrer) {
      Ok(Loaded::Collection(coll)) => coll,
      Ok(Loaded::Code(_)) => {
        unreachable!("split_name returned None but the path is a file")
      },
      Err(e) => {
        // todo: if this can actually be produced, return Err(ImportAll) instead
        let parent = abs_path.split_last().expect("import path nonzero").1;
        // exit without error if it was injected, or raise any IO error that was
        // previously swallowed
        return if is_injected_module(parent) { Ok(all) } else { Err(e) };
      },
    };
    // recurse on all files and folders within
    for item in coll.iter() {
      let abs_subpath = pushed_ref(abs_path, i.i(item));
      all = load_abs_path_rec(
        referrer,
        &abs_subpath,
        all,
        source,
        get_source,
        is_injected_module,
        ctx,
      )?;
    }
    Ok(all)
  }
}

/// Load and preparse all files reachable from the load targets via
/// imports that aren't injected.
///
/// is_injected_module must return false for injected symbols, but may return
/// true for parents of injected modules that are not directly part of the
/// injected data (the ProjectTree doesn't make a distinction between the two)
pub fn load_source<'a>(
  targets: impl Iterator<Item = &'a [Tok<String>]>,
  ctx: Context,
  get_source: &impl Fn(&[Tok<String>], &[Tok<String>]) -> IOResult,
  is_injected_module: &impl Fn(&[Tok<String>]) -> bool,
) -> ProjectResult<(Preparsed, LoadedSourceTable)> {
  let mut table = LoadedSourceTable::new();
  let mut all =
    Preparsed(Module { extra: PreExtra::Dir, entries: HashMap::new() });
  let mut any_target = false;
  for target in targets {
    any_target |= true;
    all = load_abs_path_rec(
      &[],
      target,
      all,
      &mut table,
      get_source,
      is_injected_module,
      ctx,
    )?;
  }
  if any_target { Ok((all, table)) } else { Err(NoTargets.rc()) }
}
