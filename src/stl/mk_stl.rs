use hashbrown::HashMap;
use rust_embed::RustEmbed;

use super::bool::bool;
use super::conv::conv;
use super::io::io;
use super::num::num;
use super::str::str;
use crate::interner::Interner;
use crate::pipeline::file_loader::mk_embed_cache;
use crate::pipeline::{from_const_tree, parse_layer, ProjectTree};
use crate::representations::VName;
use crate::sourcefile::{FileEntry, Import};

/// Feature flags for the STL.
#[derive(Default)]
pub struct StlOptions {
  /// Whether impure functions (such as io::debug) are allowed. An embedder
  /// would typically disable this flag
  pub impure: bool,
}

#[derive(RustEmbed)]
#[folder = "src/stl"]
#[prefix = "std/"]
#[include = "*.orc"]
struct StlEmbed;

// TODO: fix all orc modules to not rely on prelude

/// Build the standard library used by the interpreter by combining the other
/// libraries
pub fn mk_stl(i: &Interner, options: StlOptions) -> ProjectTree<VName> {
  let const_tree = from_const_tree(
    HashMap::from([(
      i.i("std"),
      io(i, options.impure) + conv(i) + bool(i) + str(i) + num(i),
    )]),
    &[i.i("std")],
  );
  let ld_cache = mk_embed_cache::<StlEmbed>(".orc", i);
  let targets = StlEmbed::iter()
    .map(|path| {
      path
        .strip_suffix(".orc")
        .expect("the embed is filtered for suffix")
        .split('/')
        .map(|segment| i.i(segment))
        .collect::<Vec<_>>()
    })
    .collect::<Vec<_>>();
  parse_layer(
    targets.iter().map(|v| &v[..]),
    &|p| ld_cache.find(p),
    &const_tree,
    &[],
    i,
  )
  .expect("Parse error in STL")
}

/// Generate prelude lines to be injected to every module compiled with the STL
pub fn mk_prelude(i: &Interner) -> Vec<FileEntry> {
  vec![FileEntry::Import(vec![Import {
    path: i.i(&[i.i("std"), i.i("prelude")][..]),
    name: None,
  }])]
}
