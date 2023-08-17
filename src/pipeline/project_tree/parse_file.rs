use std::rc::Rc;

use super::add_prelude::add_prelude;
use super::collect_ops::{collect_ops_for, ExportedOpsCache};
use super::normalize_imports::normalize_imports;
use super::prefix::prefix;
use crate::error::ProjectResult;
use crate::interner::{Interner, Tok};
use crate::parse;
use crate::pipeline::source_loader::LoadedSourceTable;
use crate::representations::sourcefile::{normalize_namespaces, FileEntry};

/// Parses a file with the correct operator set
///
/// # Panics
///
/// - on syntax error
/// - if namespaces are exported inconsistently
///
/// These are both checked in the preparsing stage
pub fn parse_file(
  path: &[Tok<String>],
  loaded: &LoadedSourceTable,
  ops_cache: &ExportedOpsCache,
  i: &Interner,
  prelude: &[FileEntry],
) -> ProjectResult<Vec<FileEntry>> {
  let ld = &loaded[path];
  // let ops_cache = collect_ops::mk_cache(loaded, i);
  let ops = collect_ops_for(path, loaded, ops_cache, i)?;
  let ops_vec = ops.iter().map(|t| i.r(*t)).cloned().collect::<Vec<_>>();
  let ctx = parse::ParsingContext {
    interner: i,
    ops: &ops_vec,
    file: Rc::new(i.extern_all(path)),
  };
  let entries = i.expect(
    parse::parse2(ld.text.as_str(), ctx),
    "This error should have been caught during loading",
  );
  let with_prelude = add_prelude(entries, path, prelude);
  let impnormalized =
    normalize_imports(&ld.preparsed.0, with_prelude, path, ops_cache, i);
  let nsnormalized = normalize_namespaces(Box::new(impnormalized.into_iter()))
    .expect("This error should have been caught during preparsing");
  let prefixed = prefix(nsnormalized, path, ops_cache, i);
  Ok(prefixed)
}
