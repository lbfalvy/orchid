use std::collections::HashMap;

use crate::representations::VName;
use crate::sourcefile::FileEntry;

#[derive(Debug)]
pub struct LoadedSource {
  pub entries: Vec<FileEntry>
}

pub type LoadedSourceTable = HashMap<VName, LoadedSource>;
