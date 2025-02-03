use std::env;
use std::ffi::OsStr;
use std::fs::{DirEntry, File};
use std::io::{self, Read};
use std::path::Path;

use crate::Args;

pub fn check_api_refs(_args: &Args) -> io::Result<()> {
	walk_wsp(&mut |_| Ok(true), &mut |file| {
		if file.path().extension() == Some(OsStr::new("rs")) && file.file_name() != "lib.rs" {
			let mut contents = String::new();
			File::open(file.path())?.read_to_string(&mut contents)?;
			for (l, line) in contents.lines().enumerate() {
				if !line.trim().starts_with("use") {
					continue;
				}
				let Some(c) = line.find("orchid_api") else { continue };
				if Some(c) == line.find("orchid_api_") {
					continue;
				}
				let dname = file.path().to_string_lossy().to_string();
				eprintln!("orchid_api imported in {dname} at {};{}", l + 1, c + 1)
			}
		}
		Ok(())
	})
}

fn walk_wsp(
	dir_filter: &mut impl FnMut(&DirEntry) -> io::Result<bool>,
	file_handler: &mut impl FnMut(DirEntry) -> io::Result<()>,
) -> io::Result<()> {
	return recurse(&env::current_dir()?, dir_filter, file_handler);
	fn recurse(
		dir: &Path,
		dir_filter: &mut impl FnMut(&DirEntry) -> io::Result<bool>,
		file_handler: &mut impl FnMut(DirEntry) -> io::Result<()>,
	) -> io::Result<()> {
		for file in dir.read_dir()?.collect::<Result<Vec<_>, _>>()? {
			if file.metadata()?.is_dir() && dir_filter(&file)? {
				recurse(&file.path(), dir_filter, file_handler)?;
			}
			file_handler(file)?;
		}
		Ok(())
	}
}
