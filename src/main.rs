#![feature(generators, generator_trait)]
#![feature(never_type)]
#![feature(unwrap_infallible)]
#![feature(arc_unwrap_or_clone)]
#![feature(hasher_prefixfree_extras)]
#![feature(closure_lifetime_binder)]
#![feature(generic_arg_infer)]
#![feature(array_chunks)]
#![feature(fmt_internals)]
#![feature(map_try_insert)]
#![feature(slice_group_by)]
#![feature(trait_alias)]
#![feature(return_position_impl_trait_in_trait)]

mod cli;
mod external;
pub(crate) mod foreign;
mod foreign_macros;
mod interner;
mod interpreter;
mod parse;
mod pipeline;
mod representations;
mod rule;
mod run_dir;
mod utils;
use std::fs::File;
use std::path::PathBuf;

use clap::Parser;
use cli::prompt;
pub use representations::ast;
use run_dir::run_dir;

/// Orchid interpreter
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
  /// Folder containing main.orc
  #[arg(short, long)]
  pub project: Option<String>,
}

fn main() {
  let args = Args::parse();
  let path = args.project.unwrap_or_else(|| {
    prompt("Enter a project root", ".".to_string(), |p| {
      let mut path: PathBuf = p.trim().into();
      path.push("main.orc");
      match File::open(&path) {
        Ok(_) => Ok(p),
        Err(e) => Err(format!("{}: {e}", path.display())),
      }
    })
  });
  run_dir(&PathBuf::try_from(path).unwrap());
}
