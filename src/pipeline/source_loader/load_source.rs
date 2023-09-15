use hashbrown::HashMap;

use super::loaded_source::{LoadedSource, LoadedSourceTable};
use super::preparse::preparse;
use super::{PreExtra, Preparsed};
use crate::error::{
  NoTargets, ProjectError, ProjectResult, UnexpectedDirectory,
};
use crate::interner::{Interner, Tok};
use crate::pipeline::file_loader::{IOResult, Loaded};
use crate::pipeline::import_abs_path::import_abs_path;
use crate::representations::sourcefile::FileEntry;
use crate::tree::Module;
use crate::utils::pure_push::pushed_ref;
use crate::utils::{split_max_prefix, unwrap_or};
use crate::Location;

/// Load the source at the given path or all within if it's a collection,
/// and all sources imported from these.
fn load_abs_path_rec(
  abs_path: &[Tok<String>],
  mut all: Preparsed,
  source: &mut LoadedSourceTable,
  prelude: &[FileEntry],
  i: &Interner,
  get_source: &impl Fn(&[Tok<String>]) -> IOResult,
  is_injected_module: &impl Fn(&[Tok<String>]) -> bool,
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
    get_source(p).map(|l| l.is_code()).unwrap_or(false)
  });
  if let Some((filename, _)) = name_split {
    // Termination: exit if entry already visited
    if source.contains_key(filename) {
      return Ok(all);
    }
    // if the filename is valid, load, preparse and record this file
    let text = unwrap_or!(get_source(filename)? => Loaded::Code; {
      return Err(UnexpectedDirectory { path: filename.to_vec() }.rc())
    });
    source.insert(filename.to_vec(), LoadedSource { text: text.clone() });
    let preparsed = preparse(filename.to_vec(), text.as_str(), prelude, i)?;
    // recurse on all imported modules
    // will be taken and returned by the closure. None iff an error is thrown
    all = preparsed.0.search_all(all, &mut |modpath,
                                             module,
                                             mut all|
     -> ProjectResult<_> {
      let details = unwrap_or!(module.extra.details(); return Ok(all));
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
          &abs_pathv,
          all,
          source,
          prelude,
          i,
          get_source,
          is_injected_module,
        )?;
      }
      Ok(all)
    })?;
    // Combine the trees
    all.0.overlay(preparsed.0).map(Preparsed)
  } else {
    // If the path is not within a file, load it as directory
    let coll = match get_source(abs_path) {
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
        &abs_subpath,
        all,
        source,
        prelude,
        i,
        get_source,
        is_injected_module,
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
  prelude: &[FileEntry],
  i: &Interner,
  get_source: &impl Fn(&[Tok<String>]) -> IOResult,
  is_injected_module: &impl Fn(&[Tok<String>]) -> bool,
) -> ProjectResult<(Preparsed, LoadedSourceTable)> {
  let mut table = LoadedSourceTable::new();
  let mut all =
    Preparsed(Module { extra: PreExtra::Dir, entries: HashMap::new() });
  let mut any_target = false;
  for target in targets {
    any_target |= true;
    all = load_abs_path_rec(
      target,
      all,
      &mut table,
      prelude,
      i,
      get_source,
      is_injected_module,
    )?;
  }
  if any_target { Ok((all, table)) } else { Err(NoTargets.rc()) }
}
