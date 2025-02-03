mod check_api_refs;
mod orcx;

use std::io;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};

use check_api_refs::check_api_refs;
use clap::{Parser, Subcommand};
use orcx::orcx;

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
	Orcx {
		#[arg(trailing_var_arg = true, num_args = 1..)]
		orcx_argv: Vec<String>,
	},
}

pub static EXIT_OK: AtomicBool = AtomicBool::new(true);

fn main() -> io::Result<ExitCode> {
	let args = Args::parse();
	match &args.command {
		Commands::CheckApiRefs => check_api_refs(&args)?,
		Commands::Orcx { orcx_argv } => orcx(&args, orcx_argv)?,
	}
	Ok(if EXIT_OK.load(Ordering::Relaxed) { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}
