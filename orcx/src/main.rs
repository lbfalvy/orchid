use std::fs::File;
use std::io::Read;
use std::process::Command;
use std::sync::Arc;

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use itertools::Itertools;
use orchid_base::interner::intern;
use orchid_base::logging::{LogStrategy, Logger};
use orchid_base::tree::ttv_fmt;
use orchid_host::extension::{init_systems, Extension};
use orchid_host::lex::lex;
use orchid_host::subprocess::Subprocess;

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
  Lex {
    #[arg(short, long)]
    file: Utf8PathBuf,
  },
}

fn main() {
  let args = Args::parse();
  let logger = Logger::new(LogStrategy::StdErr);
  match args.command {
    Commands::Lex { file } => {
      let extensions = (args.extension.iter())
        .map(|f| Subprocess::new(Command::new(f.as_os_str()), logger.clone()).unwrap())
        .map(|cmd| Extension::new_process(Arc::new(cmd), logger.clone()).unwrap())
        .collect_vec();
      let systems = init_systems(&args.system, &extensions).unwrap();
      let mut file = File::open(file.as_std_path()).unwrap();
      let mut buf = String::new();
      file.read_to_string(&mut buf).unwrap();
      let lexemes = lex(intern(&buf), &systems).unwrap();
      println!("{}", ttv_fmt(&lexemes))
    },
  }
}
