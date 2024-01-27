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
pub mod gen;
pub mod intermediate;
pub mod interpreter;
pub mod libs;
pub mod location;
pub mod name;
pub mod parse;
pub mod pipeline;
pub mod rule;
pub mod tree;
pub mod utils;
pub mod virt_fs;
