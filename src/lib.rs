#![warn(missing_docs)]
#![doc(
  html_logo_url = "https://raw.githubusercontent.com/lbfalvy/orchid/master/icon.svg"
)]
#![doc(
  html_favicon_url = "https://raw.githubusercontent.com/lbfalvy/orchid/master/icon.svg"
)]
//! Orchid is a lazy, pure scripting language to be embedded in Rust
//! applications. Check out the repo for examples and other links.
pub mod error;
pub mod facade;
pub mod foreign;
pub mod interner;
pub mod interpreter;
pub mod parse;
pub mod pipeline;
mod representations;
pub mod rule;
pub mod systems;
mod utils;

pub use interner::{Interner, Tok};
pub use pipeline::file_loader::{mk_dir_cache, mk_embed_cache};
pub use pipeline::parse_layer;
/// Element of VName and a common occurrence in the API
pub type Stok = Tok<String>;
pub use representations::ast_to_interpreted::ast_to_interpreted;
pub use representations::project::{
  collect_consts, collect_rules, vname_to_sym_tree, ProjectTree,
};
pub use representations::{
  ast, from_const_tree, interpreted, sourcefile, tree, ConstTree, Location,
  NameLike, OrcString, PathSet, Sym, VName,
};
pub use utils::substack::Substack;
pub use utils::{ddispatch, take_with_output, thread_pool, IdMap, Side};
