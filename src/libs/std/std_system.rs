//! Add the standard library's constants and mcacros to an Orchid environment

#![allow(non_upper_case_globals)]

use rust_embed::RustEmbed;

use super::binary::bin_lib;
use super::bool::bool_lib;
use super::conv::conv_lib;
use super::exit_status::exit_status_lib;
use super::inspect::inspect_lib;
use super::number::num_lib;
use super::panic::panic_lib;
use super::protocol::{parsers, protocol_lib};
use super::reflect::reflect_lib;
use super::state::{state_handlers, state_lib};
use super::string::{str_lib, StringLexer};
use super::tuple::tuple_lib;
use crate::facade::system::{IntoSystem, System};
use crate::gen::tree::{ConstCombineErr, ConstTree};
use crate::location::CodeGenInfo;
use crate::pipeline::load_project::Prelude;
use crate::tree::ModEntry;
use crate::utils::combine::Combine;
use crate::virt_fs::{EmbeddedFS, VirtFS};
use crate::{sym, vname};

#[derive(RustEmbed)]
#[folder = "src/libs/std"]
#[include = "*.orc"]
struct StdEmbed;

/// Feature flags for the STL.
#[derive(Default)]
pub struct StdConfig {
  /// Whether impure functions (such as io::debug) are allowed. An embedder
  /// would typically disable this flag
  pub impure: bool,
}
impl StdConfig {
  fn stdlib(&self) -> Result<ConstTree, ConstCombineErr> {
    let pure_tree = tuple_lib()
      .combine(bin_lib())?
      .combine(bool_lib())?
      .combine(conv_lib())?
      .combine(exit_status_lib())?
      .combine(num_lib())?
      .combine(panic_lib())?
      .combine(protocol_lib())?
      .combine(reflect_lib())?
      .combine(state_lib())?
      .combine(str_lib())?;
    if !self.impure {
      return Ok(pure_tree);
    }
    pure_tree.combine(inspect_lib())
  }
}

impl IntoSystem<'static> for StdConfig {
  fn into_system(self) -> System<'static> {
    System {
      name: "stdlib",
      constants: self.stdlib().expect("stdlib tree is malformed"),
      code: ModEntry::ns("std", [ModEntry::leaf(
        EmbeddedFS::new::<StdEmbed>(".orc", CodeGenInfo::no_details(sym!(std::fs))).rc(),
      )]),
      prelude: vec![Prelude {
        target: vname!(std::prelude),
        exclude: vname!(std),
        owner: CodeGenInfo::no_details(sym!(std::prelude)),
      }],
      handlers: state_handlers(),
      lexer_plugins: vec![Box::new(StringLexer)],
      line_parsers: parsers(),
    }
  }
}
