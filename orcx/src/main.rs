use std::{fs::File, io::Read, process::Command};

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use itertools::Itertools;
use orchid_base::{interner::intern, logging::{LogStrategy, Logger}};
use orchid_host::{extension::{init_systems, Extension}, lex::lex, tree::fmt_tt_v};

#[derive(Parser, Debug)]
#[command(version, about, long_about)]
pub struct Args {
  #[arg(short, long)]
  extension: Vec<Utf8PathBuf>,
  #[arg(short, long)]
  system: Vec<String>,
  #[command(subcommand)]
  command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
  Lex{
    #[arg(short, long)]
    file: Utf8PathBuf
  },
}

fn main() {
  let args = Args::parse();
  match args.command {
    Commands::Lex { file } => {
      let extensions = (args.extension.iter())
        .map(|f| Extension::new(Command::new(f.as_os_str()), Logger::new(LogStrategy::StdErr)).unwrap())
        .collect_vec();
      let systems = init_systems(&args.system, &extensions).unwrap();
      let mut file = File::open(file.as_std_path()).unwrap();
      let mut buf = String::new();
      file.read_to_string(&mut buf).unwrap();
      let lexemes = lex(intern(&buf), &systems).unwrap();
      println!("{}", fmt_tt_v(&lexemes))
    }
  }
}
