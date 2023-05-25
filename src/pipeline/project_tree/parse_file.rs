use std::rc::Rc;

use super::add_prelude::add_prelude;
use super::collect_ops::{collect_ops_for, ExportedOpsCache};
use super::normalize_imports::normalize_imports;
use super::prefix::prefix;
use crate::interner::{Interner, Sym};
use crate::parse;
use crate::pipeline::error::ProjectError;
use crate::pipeline::source_loader::LoadedSourceTable;
use crate::representations::sourcefile::{normalize_namespaces, FileEntry};

pub fn parse_file(
  path: Sym,
  loaded: &LoadedSourceTable,
  ops_cache: &ExportedOpsCache,
  i: &Interner,
  prelude: &[FileEntry],
) -> Result<Vec<FileEntry>, Rc<dyn ProjectError>> {
  let ld = &loaded[&path];
  // let ops_cache = collect_ops::mk_cache(loaded, i);
  let ops = collect_ops_for(&i.r(path)[..], loaded, ops_cache, i)?;
  let ops_vec = ops.iter().map(|t| i.r(*t)).cloned().collect::<Vec<_>>();
  let ctx = parse::ParsingContext {
    interner: i,
    ops: &ops_vec,
    file: Rc::new(i.extern_vec(path)),
  };
  let entries = parse::parse(ld.text.as_str(), ctx)
    .expect("This error should have been caught during loading");
  let with_prelude = add_prelude(entries, &i.r(path)[..], prelude);
  let impnormalized = normalize_imports(
    &ld.preparsed.0,
    with_prelude,
    &i.r(path)[..],
    ops_cache,
    i,
  );
  let nsnormalized = normalize_namespaces(Box::new(impnormalized.into_iter()))
    .expect("This error should have been caught during preparsing");
  let prefixed = prefix(nsnormalized, &i.r(path)[..], ops_cache, i);
  Ok(prefixed)
}
