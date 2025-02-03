use std::io;
use std::process::Command;
use std::sync::atomic::Ordering;

use crate::{Args, EXIT_OK};

pub fn orcx(_args: &Args, argv: &[String]) -> io::Result<()> {
	if !Command::new("cargo").args(["build", "-p", "orchid-std"]).status()?.success() {
		EXIT_OK.store(false, Ordering::Relaxed);
		return Ok(());
	}
	if !Command::new("cargo").args(["run", "-p", "orcx", "--"]).args(argv).status()?.success() {
		EXIT_OK.store(false, Ordering::Relaxed);
	}
	Ok(())
}
