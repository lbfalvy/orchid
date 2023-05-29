#![deny(missing_docs)]
#![doc(
  html_logo_url = "https://raw.githubusercontent.com/lbfalvy/orchid/master/icon.svg"
)]
#![doc(
  html_favicon_url = "https://raw.githubusercontent.com/lbfalvy/orchid/master/icon.svg"
)]
//! Orchid is a lazy, pure scripting language to be embedded in Rust
//! applications. Check out the repo for examples and other links.
pub mod foreign;
mod foreign_macros;
pub mod interner;
pub mod interpreter;
mod parse;
pub mod pipeline;
mod representations;
pub mod rule;
pub mod stl;
mod utils;

pub use representations::ast_to_interpreted::ast_to_interpreted;
pub use representations::{
  ast, interpreted, sourcefile, tree, Literal, Location, PathSet, Primitive,
};
pub use utils::{Side, Substack};
