#![allow(non_upper_case_globals)]
use hashbrown::HashMap;
use rust_embed::RustEmbed;

use super::bin::bin;
use super::bool::bool;
use super::conv::conv;
use super::inspect::inspect;
use super::num::num;
use super::panic::panic;
use super::state::{state_handlers, state_lib};
use super::str::str;
use crate::facade::{IntoSystem, System};
use crate::interner::Interner;
use crate::pipeline::file_loader::embed_to_map;
use crate::sourcefile::{FileEntry, FileEntryKind, Import};
use crate::Location;

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

impl IntoSystem<'static> for StlConfig {
  fn into_system(self, i: &Interner) -> System<'static> {
    let pure_fns =
      conv(i) + bool(i) + str(i) + num(i) + bin(i) + panic(i) + state_lib(i);
    let mk_impure_fns = || inspect(i);
    let fns = if self.impure { pure_fns + mk_impure_fns() } else { pure_fns };
    System {
      name: vec!["std".to_string()],
      constants: HashMap::from([(i.i("std"), fns)]),
      code: embed_to_map::<StlEmbed>(".orc", i),
      prelude: vec![FileEntry {
        locations: vec![Location::Unknown],
        kind: FileEntryKind::Import(vec![Import {
          location: Location::Unknown,
          path: vec![i.i("std"), i.i("prelude")],
          name: None,
        }]),
      }],
      handlers: state_handlers(),
    }
  }
}