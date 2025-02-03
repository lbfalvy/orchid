use std::io;
use std::process::Command;
use std::sync::atomic::Ordering;

use crate::{Args, EXIT_OK};

pub fn orcx(_args: &Args, argv: &[String]) -> io::Result<()> {
	if !Command::new("cargo").args(["build", "-p", "orchid-std"]).status()?.success() {
		EXIT_OK.store(false, Ordering::Relaxed);
		return Ok(());
	}
	let status = Command::new("cargo")
		.args("run -p orcx --".split(' ').chain(argv.iter().map(|s| s.as_str())))
		.status()?;
	EXIT_OK.store(status.success(), Ordering::Relaxed);
	Ok(())
}
