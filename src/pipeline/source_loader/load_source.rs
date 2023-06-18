use std::iter;
use std::rc::Rc;

use super::loaded_source::{LoadedSource, LoadedSourceTable};
use super::preparse::preparse;
use crate::interner::{Interner, Tok};
use crate::pipeline::error::{ProjectError, UnexpectedDirectory};
use crate::pipeline::file_loader::{IOResult, Loaded};
use crate::pipeline::import_abs_path::import_abs_path;
use crate::representations::sourcefile::FileEntry;
use crate::utils::{split_max_prefix, unwrap_or};

/// Load the source at the given path or all within if it's a collection,
/// and all sources imported from these.
fn load_abs_path_rec(
  abs_path: &[Tok<String>],
  table: &mut LoadedSourceTable,
  prelude: &[FileEntry],
  i: &Interner,
  get_source: &impl Fn(&[Tok<String>]) -> IOResult,
  is_injected_module: &impl Fn(&[Tok<String>]) -> bool,
) -> Result<(), Rc<dyn ProjectError>> {
  // # Termination
  //
  // Every recursion of this function either
  // - adds one of the files in the source directory to `table` or
  // - recursively traverses a directory tree
  // therefore eventually the function exits, assuming that the directory tree
  // contains no cycles.

  // Termination: exit if entry already visited
  if table.contains_key(abs_path) {
    return Ok(());
  }

  // try splitting the path to file, swallowing any IO errors
  let name_split = split_max_prefix(abs_path, &|p| {
    get_source(p).map(|l| l.is_code()).unwrap_or(false)
  });
  if let Some((filename, _)) = name_split {
    // if the filename is valid, load, preparse and record this file
    let text = unwrap_or!(get_source(filename)? => Loaded::Code; {
      return Err(UnexpectedDirectory { path: i.extern_all(filename) }.rc())
    });
    let preparsed = preparse(
      filename.iter().map(|t| i.r(*t)).cloned().collect(),
      text.as_str(),
      prelude,
      i,
    )?;
    table.insert(filename.to_vec(), LoadedSource {
      text,
      preparsed: preparsed.clone(),
    });
    // recurse on all imported modules
    preparsed.0.visit_all_imports(&mut |modpath, _module, import| {
      let abs_pathv =
        import_abs_path(filename, modpath, &import.nonglob_path(i), i)?;
      // recurse on imported module
      load_abs_path_rec(
        &abs_pathv,
        table,
        prelude,
        i,
        get_source,
        is_injected_module,
      )
    })
  } else {
    // If the path is not within a file, load it as directory
    let coll = match get_source(abs_path) {
      Ok(Loaded::Collection(coll)) => coll,
      Ok(Loaded::Code(_)) =>
        unreachable!("split_name returned None but the path is a file"),
      Err(e) => {
        let parent = abs_path.split_last().expect("import path nonzero").1;
        // exit without error if it was injected, or raise any IO error that was
        // previously swallowed
        return if is_injected_module(parent) { Ok(()) } else { Err(e) };
      },
    };
    // recurse on all files and folders within
    for item in coll.iter() {
      let abs_subpath = (abs_path.iter())
        .copied()
        .chain(iter::once(i.i(item)))
        .collect::<Vec<_>>();
      load_abs_path_rec(
        &abs_subpath,
        table,
        prelude,
        i,
        get_source,
        is_injected_module,
      )?
    }
    Ok(())
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
) -> Result<LoadedSourceTable, Rc<dyn ProjectError>> {
  let mut table = LoadedSourceTable::new();
  for target in targets {
    load_abs_path_rec(
      target,
      &mut table,
      prelude,
      i,
      get_source,
      is_injected_module,
    )?
  }
  Ok(table)
}
