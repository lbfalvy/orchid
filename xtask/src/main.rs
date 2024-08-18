use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::{Parser, Subcommand};

#[derive(Parser)]
pub struct Args {
  #[arg(short, long)]
  verbose: bool,
  #[command(subcommand)]
  command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
  CheckApiRefs,
}

pub static EXIT_OK: AtomicBool = AtomicBool::new(true);

fn main() -> io::Result<ExitCode> {
  let args = Args::parse();
  match args.command {
    Commands::CheckApiRefs => check_api_refs(&args, env::current_dir()?)?,
  }
  Ok(if EXIT_OK.load(Ordering::Relaxed) { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

fn check_api_refs(args: &Args, dir: PathBuf) -> io::Result<()> {
  for file in dir.read_dir()?.collect::<Result<Vec<_>, _>>()? {
    if args.verbose {
      eprintln!("Checking {}", file.path().to_string_lossy());
    }
    if file.metadata()?.is_dir() {
      check_api_refs(args, file.path())?
    }
    if file.path().extension() != Some(OsStr::new("rs")) || file.file_name() == "lib.rs" {
      continue;
    }
    let mut contents = String::new();
    File::open(file.path())?.read_to_string(&mut contents)?;
    for (l, line) in contents.lines().enumerate() {
      if line.trim().starts_with("use") {
        if let Some(c) = line.find("orchid_api") {
          if Some(c) != line.find("orchid_api_") {
            let dname = file.path().to_string_lossy().to_string();
            eprintln!("orchid_api imported in {dname} at {};{}", l + 1, c + 1)
          }
        }
      }
    }
  }
  Ok(())
}
