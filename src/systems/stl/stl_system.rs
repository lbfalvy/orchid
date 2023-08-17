#![allow(non_upper_case_globals)]
use hashbrown::HashMap;
use rust_embed::RustEmbed;

use super::bin::bin;
use super::bool::bool;
use super::conv::conv;
use super::inspect::inspect;
use super::num::num;
use super::panic::panic;
use super::str::str;
use crate::facade::{IntoSystem, System};
use crate::interner::Interner;
use crate::interpreter::HandlerTable;
use crate::pipeline::file_loader::embed_to_map;
use crate::sourcefile::{FileEntry, Import};

/// Feature flags for the STL.
#[derive(Default)]
pub struct StlConfig {
  /// Whether impure functions (such as io::debug) are allowed. An embedder
  /// would typically disable this flag
  pub impure: bool,
}

#[derive(RustEmbed)]
#[folder = "src/systems/stl"]
#[prefix = "std/"]
#[include = "*.orc"]
struct StlEmbed;

// TODO: fix all orc modules to not rely on prelude

impl IntoSystem<'static> for StlConfig {
  fn into_system(self, i: &Interner) -> System<'static> {
    let pure_fns = conv(i) + bool(i) + str(i) + num(i) + bin(i) + panic(i);
    let mk_impure_fns = || inspect(i);
    let fns = if self.impure { pure_fns + mk_impure_fns() } else { pure_fns };
    System {
      name: vec!["std".to_string()],
      constants: HashMap::from([(i.i("std"), fns)]),
      code: embed_to_map::<StlEmbed>(".orc", i),
      prelude: vec![FileEntry::Import(vec![Import {
        path: i.i(&[i.i("std"), i.i("prelude")][..]),
        name: None,
      }])],
      handlers: HandlerTable::new(),
    }
  }
}
