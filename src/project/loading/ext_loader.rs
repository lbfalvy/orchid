use crate::parse::FileEntry;

use super::{Loader, Loaded};

pub fn ext_loader(data: Vec<FileEntry>) -> impl Loader {
  move |_: &[&str]| Ok(Loaded::External(data.clone()))
}