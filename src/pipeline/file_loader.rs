//! Source loader callback definition and builtin implementations
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{fs, io};

use hashbrown::{HashMap, HashSet};
use rust_embed::RustEmbed;

use crate::error::{ProjectError, ProjectResult};
#[allow(unused)] // for doc
use crate::facade::System;
use crate::interner::Interner;
use crate::utils::Cache;
use crate::{Location, Stok, VName};

/// All the data available about a failed source load call
#[derive(Debug)]
pub struct FileLoadingError {
  file: io::Error,
  dir: io::Error,
  path: Vec<String>,
}
impl ProjectError for FileLoadingError {
  fn description(&self) -> &str {
    "Neither a file nor a directory could be read from the requested path"
  }
  fn one_position(&self, _i: &Interner) -> crate::Location {
    Location::File(Rc::new(self.path.clone()))
  }
  fn message(&self, _i: &Interner) -> String {
    format!("File: {}\nDirectory: {}", self.file, self.dir)
  }
}

/// Represents the result of loading code from a string-tree form such
/// as the file system.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Loaded {
  /// Conceptually equivalent to a sourcefile
  Code(Rc<String>),
  /// Conceptually equivalent to the list of *.orc files in a folder, without
  /// the extension
  Collection(Rc<Vec<String>>),
}
impl Loaded {
  /// Is the loaded item source code (not a collection)?
  pub fn is_code(&self) -> bool {
    matches!(self, Loaded::Code(_))
  }
}

/// Returned by any source loading callback
pub type IOResult = ProjectResult<Loaded>;

/// Load a file from a path expressed in Rust strings, but relative to
/// a root expressed as an OS Path.
pub fn load_file(root: &Path, path: &[impl AsRef<str>]) -> IOResult {
  let full_path = path.iter().fold(root.to_owned(), |p, s| p.join(s.as_ref()));
  let file_path = full_path.with_extension("orc");
  let file_error = match fs::read_to_string(file_path) {
    Ok(string) => return Ok(Loaded::Code(Rc::new(string))),
    Err(err) => err,
  };
  let dir = match fs::read_dir(&full_path) {
    Ok(dir) => dir,
    Err(dir_error) =>
      return Err(
        FileLoadingError {
          file: file_error,
          dir: dir_error,
          path: path.iter().map(|s| s.as_ref().to_string()).collect(),
        }
        .rc(),
      ),
  };
  let names = dir
    .filter_map(Result::ok)
    .filter_map(|ent| {
      let fname = ent.file_name().into_string().ok()?;
      let ftyp = ent.metadata().ok()?.file_type();
      Some(if ftyp.is_dir() {
        fname
      } else {
        fname.strip_suffix(".or")?.to_string()
      })
    })
    .collect();
  Ok(Loaded::Collection(Rc::new(names)))
}

/// Generates a cached file loader for a directory
pub fn mk_dir_cache(root: PathBuf, i: &Interner) -> Cache<VName, IOResult> {
  Cache::new(move |vname: VName, _this| -> IOResult {
    let path = vname.iter().map(|t| i.r(*t).as_str()).collect::<Vec<_>>();
    load_file(&root, &path)
  })
}

/// Load a file from the specified path from an embed table
///
/// # Panics
///
/// if the `RustEmbed` includes files that do not end in `ext`
pub fn load_embed<T: 'static + RustEmbed>(path: &str, ext: &str) -> IOResult {
  let file_path = path.to_string() + ext;
  if let Some(file) = T::get(&file_path) {
    let s =
      String::from_utf8(file.data.to_vec()).expect("Embed must be valid UTF-8");
    Ok(Loaded::Code(Rc::new(s)))
  } else {
    let entries = T::iter()
      .map(|c| c.to_string())
      .filter_map(|path: String| {
        let item_prefix = path.to_string() + "/";
        path.strip_prefix(&item_prefix).map(|subpath| {
          let item_name = subpath
            .split_inclusive('/')
            .next()
            .expect("Exact match excluded earlier");
          item_name
            .strip_suffix('/') // subdirectory
            .or_else(|| item_name.strip_suffix(ext)) // file
            .expect("embed should be filtered to extension")
            .to_string()
        })
      })
      .collect::<Vec<String>>();
    Ok(Loaded::Collection(Rc::new(entries)))
  }
}

/// Generates a cached file loader for a [RustEmbed]
pub fn mk_embed_cache<'a, T: 'static + RustEmbed>(
  ext: &'a str,
  i: &'a Interner,
) -> Cache<'a, Vec<Stok>, IOResult> {
  Cache::new(move |vname: VName, _this| -> IOResult {
    let path = i.extern_all(&vname).join("/");
    load_embed::<T>(&path, ext)
  })
}

/// Load all files from an embed and convert them into a map usable in a
/// [System]
pub fn embed_to_map<T: 'static + RustEmbed>(
  suffix: &str,
  i: &Interner,
) -> HashMap<Vec<Stok>, Loaded> {
  let mut files = HashMap::new();
  let mut dirs = HashMap::new();
  for path in T::iter() {
    let vpath = path
      .strip_suffix(suffix)
      .expect("the embed must be filtered for suffix")
      .split('/')
      .map(|s| s.to_string())
      .collect::<Vec<_>>();
    let tokvpath = vpath.iter().map(|segment| i.i(segment)).collect::<Vec<_>>();
    let data = T::get(&path).expect("path from iterator").data;
    let text =
      String::from_utf8(data.to_vec()).expect("code embeds must be utf-8");
    files.insert(tokvpath.clone(), text);
    for (lvl, subname) in vpath.iter().enumerate() {
      let dirname = tokvpath.split_at(lvl).0;
      let (_, entries) = (dirs.raw_entry_mut().from_key(dirname))
        .or_insert_with(|| (dirname.to_vec(), HashSet::new()));
      entries.get_or_insert_with(subname, Clone::clone);
    }
  }
  (files.into_iter())
    .map(|(k, s)| (k, Loaded::Code(Rc::new(s))))
    .chain((dirs.into_iter()).map(|(k, entv)| {
      (k, Loaded::Collection(Rc::new(entv.into_iter().collect())))
    }))
    .collect()
}
