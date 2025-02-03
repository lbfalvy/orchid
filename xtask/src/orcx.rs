use std::io;
use std::process::Command;
use std::sync::atomic::Ordering;

use crate::{Args, EXIT_OK};

pub fn orcx(_args: &Args, subcommand: &[String]) -> io::Result<()> {
	eprintln!("running orcx {}", subcommand.join(" "));
	let status = Command::new("cargo").args(["build", "-p", "orchid-std"]).status()?;
	if status.success() {
		let status = Command::new("cargo")
			.args(["run", "-p", "orcx", "--"].into_iter().chain(subcommand.iter().map(|s| s.as_str())))
			.status()?;
		EXIT_OK.store(status.success(), Ordering::Relaxed);
	} else {
		EXIT_OK.store(false, Ordering::Relaxed);
	}
	Ok(())
}
