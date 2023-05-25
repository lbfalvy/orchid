use std::iter;
use std::rc::Rc;

use super::loaded_source::{LoadedSource, LoadedSourceTable};
use super::preparse::preparse;
use crate::interner::{Interner, Sym, Tok};
use crate::pipeline::error::ProjectError;
use crate::pipeline::file_loader::{load_text, IOResult, Loaded};
use crate::pipeline::import_abs_path::import_abs_path;
use crate::pipeline::split_name::split_name;
use crate::representations::sourcefile::FileEntry;

/// Load the source at the given path or all within if it's a collection,
/// and all sources imported from these.
fn load_abs_path_rec(
  abs_path: Sym,
  table: &mut LoadedSourceTable,
  prelude: &[FileEntry],
  i: &Interner,
  get_source: &impl Fn(Sym) -> IOResult,
  is_injected: &impl Fn(&[Tok<String>]) -> bool,
) -> Result<(), Rc<dyn ProjectError>> {
  let abs_pathv = i.r(abs_path);
  // short-circuit if this import is defined externally or already known
  if is_injected(abs_pathv) | table.contains_key(&abs_path) {
    return Ok(());
  }
  // try splitting the path to file, swallowing any IO errors
  let is_file = |p| (get_source)(p).map(|l| l.is_code()).unwrap_or(false);
  let name_split = split_name(abs_pathv, &|p| is_file(i.i(p)));
  let filename = if let Some((f, _)) = name_split {
    f
  } else {
    // If the path could not be split to file, load it as directory
    let coll = if let Loaded::Collection(c) = (get_source)(abs_path)? {
      c
    }
    // ^^ raise any IO error that was previously swallowed
    else {
      panic!("split_name returned None but the path is a file")
    };
    // recurse on all files and folders within
    for item in coll.iter() {
      let abs_subpath = abs_pathv
        .iter()
        .copied()
        .chain(iter::once(i.i(item)))
        .collect::<Vec<_>>();
      load_abs_path_rec(
        i.i(&abs_subpath),
        table,
        prelude,
        i,
        get_source,
        is_injected,
      )?
    }
    return Ok(());
  };
  // otherwise load, preparse and record this file
  let text = load_text(i.i(filename), &get_source, i)?;
  let preparsed = preparse(
    filename.iter().map(|t| i.r(*t)).cloned().collect(),
    text.as_str(),
    prelude,
    i,
  )?;
  table.insert(abs_path, LoadedSource { text, preparsed: preparsed.clone() });
  // recurse on all imported modules
  preparsed.0.visit_all_imports(&mut |modpath, _module, import| {
    let abs_pathv =
      import_abs_path(filename, modpath, &import.nonglob_path(i), i)?;
    // recurse on imported module
    load_abs_path_rec(
      i.i(&abs_pathv),
      table,
      prelude,
      i,
      get_source,
      is_injected,
    )
  })
}

/// Load and preparse all files reachable from the load targets via
/// imports that aren't injected.
pub fn load_source(
  targets: &[Sym],
  prelude: &[FileEntry],
  i: &Interner,
  get_source: &impl Fn(Sym) -> IOResult,
  is_injected: &impl Fn(&[Tok<String>]) -> bool,
) -> Result<LoadedSourceTable, Rc<dyn ProjectError>> {
  let mut table = LoadedSourceTable::new();
  for target in targets {
    load_abs_path_rec(*target, &mut table, prelude, i, get_source, is_injected)?
  }
  Ok(table)
}
