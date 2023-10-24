//! Building blocks of a source file
use std::fmt::Display;

use itertools::{Either, Itertools};

use super::namelike::VName;
use crate::ast::{Constant, Rule};
use crate::error::{ProjectError, ProjectResult, TooManySupers};
use crate::interner::{Interner, Tok};
use crate::utils::pure_seq::pushed;
use crate::utils::BoxedIter;
use crate::Location;

/// An import pointing at another module, either specifying the symbol to be
/// imported or importing all available symbols with a globstar (*)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Import {
  /// Import path, a sequence of module names. Can either start with
  ///
  /// - `self` to reference the current module
  /// - any number of `super` to reference the parent module of the implied
  ///   `self`
  /// - a root name
  pub path: VName,
  /// If name is None, this is a wildcard import
  pub name: Option<Tok<String>>,
  /// Location of the final name segment, which uniquely identifies this name
  pub location: Location,
}
impl Import {
  /// Get the preload target space for this import - the prefix below
  /// which all files should be included in the compilation
  ///
  /// Returns the path if this is a glob import, or the path plus the
  /// name if this is a specific import
  #[must_use]
  pub fn nonglob_path(&self) -> VName {
    let mut path_vec = self.path.clone();
    if let Some(n) = &self.name {
      path_vec.push(n.clone())
    }
    path_vec
  }
}

impl Display for Import {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let paths = self.path.iter().map(|t| &**t).join("::");
    let names = self.name.as_ref().map(|t| t.as_str()).unwrap_or("*");
    write!(f, "{paths}::{names}")
  }
}

/// A namespace block
#[derive(Debug, Clone)]
pub struct ModuleBlock {
  /// Name prefixed to all names in the block
  pub name: Tok<String>,
  /// Prefixed entries
  pub body: Vec<FileEntry>,
}

impl Display for ModuleBlock {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let bodys = self.body.iter().map(|e| e.to_string()).join("\n");
    write!(f, "module {} {{\n{}\n}}", self.name, bodys)
  }
}

/// see [Member]
#[derive(Debug, Clone)]
pub enum MemberKind {
  /// A substitution rule. Rules apply even when they're not in scope, if the
  /// absolute names are present eg. because they're produced by other rules
  Rule(Rule<VName>),
  /// A constant (or function) associated with a name
  Constant(Constant),
  /// A prefixed set of other entries
  Module(ModuleBlock),
}
impl MemberKind {
  /// Convert to [FileEntry]
  pub fn to_entry(self, exported: bool, location: Location) -> FileEntry {
    FileEntryKind::Member(Member { exported, kind: self }).wrap(location)
  }
}

impl Display for MemberKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Constant(c) => c.fmt(f),
      Self::Module(m) => m.fmt(f),
      Self::Rule(r) => r.fmt(f),
    }
  }
}

/// Things that may be prefixed with an export
/// see [MemberKind]
#[derive(Debug, Clone)]
pub struct Member {
  /// Various members
  pub kind: MemberKind,
  /// Whether this member is exported or not
  pub exported: bool,
}

impl Display for Member {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self { exported: true, kind } => write!(f, "export {kind}"),
      Self { exported: false, kind } => write!(f, "{kind}"),
    }
  }
}

/// See [FileEntry]
#[derive(Debug, Clone)]
pub enum FileEntryKind {
  /// Imports one or all names in a module
  Import(Vec<Import>),
  /// Comments are kept here in case dev tooling wants to parse documentation
  Comment(String),
  /// An element with visibility information
  Member(Member),
  /// A list of tokens exported explicitly. This can also create new exported
  /// tokens that the local module doesn't actually define a role for
  Export(Vec<(Tok<String>, Location)>),
}
impl FileEntryKind {
  /// Wrap with no location
  pub fn wrap(self, location: Location) -> FileEntry {
    FileEntry { kind: self, locations: vec![location] }
  }
}

impl Display for FileEntryKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Comment(s) => write!(f, "--[{s}]--"),
      Self::Export(s) => {
        write!(f, "export ::({})", s.iter().map(|t| &**t.0).join(", "))
      },
      Self::Member(member) => write!(f, "{member}"),
      Self::Import(i) => {
        write!(f, "import ({})", i.iter().map(|i| i.to_string()).join(", "))
      },
    }
  }
}

/// Anything the parser might encounter in a file. See [FileEntryKind]
#[derive(Debug, Clone)]
pub struct FileEntry {
  /// What we encountered
  pub kind: FileEntryKind,
  /// Where we encountered it
  pub locations: Vec<Location>,
}

impl Display for FileEntry {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.kind.fmt(f)
  }
}

/// Summarize all imports from a file in a single list of qualified names
pub fn imports<'a>(
  src: impl Iterator<Item = &'a FileEntry> + 'a,
) -> impl Iterator<Item = &'a Import> + 'a {
  src
    .filter_map(|ent| match &ent.kind {
      FileEntryKind::Import(impv) => Some(impv.iter()),
      _ => None,
    })
    .flatten()
}

/// Join the various redeclarations of namespaces.
/// Error if they're inconsistently exported
pub fn normalize_namespaces(
  src: BoxedIter<FileEntry>,
) -> Result<Vec<FileEntry>, VName> {
  let (mut namespaces, mut rest) = src
    .partition_map::<Vec<_>, Vec<_>, _, _, _>(|ent| {
      match ent {
        FileEntry {
          kind: FileEntryKind::Member(Member {
        kind: MemberKind::Module(ns),
        exported,
      }),
          locations
        } => Either::Left((exported, ns, locations)),
        ent => Either::Right(ent)
      }
    });
  // Combine namespace blocks with the same name
  namespaces.sort_unstable_by_key(|(_, ns, _)| ns.name.clone());
  let mut lumped = namespaces
    .into_iter()
    .group_by(|(_, ns, _)| ns.name.clone())
    .into_iter()
    .map(|(name, grp)| {
      let mut exported = false;
      let mut internal = false;
      let mut grouped_source = Vec::new();
      let mut locations = Vec::new();
      for (inst_exported, ns, locs) in grp {
        if inst_exported {
          exported = true
        } else {
          internal = true
        };
        grouped_source.extend(ns.body.into_iter());
        locations.extend(locs.into_iter());
      }
      if exported == internal {
        debug_assert!(exported && internal, "Both false is impossible");
        return Err(vec![name]);
      }
      // Apply the function to the contents of these blocks too
      let body = normalize_namespaces(Box::new(grouped_source.into_iter()))
        .map_err(|e| pushed(e, name.clone()))?;
      let kind = MemberKind::Module(ModuleBlock { name, body });
      let kind = FileEntryKind::Member(Member { kind, exported });
      Ok(FileEntry { kind, locations })
    })
    .collect::<Result<Vec<_>, _>>()?;
  rest.append(&mut lumped);
  Ok(rest)
}

/// Turn a relative (import) path into an absolute path.
/// If the import path is empty, the return value is also empty.
///
/// # Errors
///
/// if the relative path contains more `super` segments than the length
/// of the absolute path.
pub fn absolute_path(
  abs_location: &[Tok<String>],
  rel_path: &[Tok<String>],
  i: &Interner,
  location: &Location,
) -> ProjectResult<VName> {
  absolute_path_rec(abs_location, rel_path, i).ok_or_else(|| {
    TooManySupers { path: rel_path.to_vec(), location: location.clone() }.rc()
  })
}

#[must_use = "this could be None which means that there are too many supers"]
fn absolute_path_rec(
  mut abs_location: &[Tok<String>],
  mut rel_path: &[Tok<String>],
  i: &Interner,
) -> Option<VName> {
  let mut relative = false;
  while rel_path.first() == Some(&i.i("super")) {
    abs_location = abs_location.split_last()?.1;
    rel_path = rel_path.split_first().expect("checked above").1;
    relative = true;
  }
  if rel_path.first() == Some(&i.i("self")) {
    relative = true;
    rel_path = rel_path.split_first().expect("checked above").1;
  }
  match relative {
    true => Some(abs_location.iter().chain(rel_path).cloned().collect()),
    false => Some(rel_path.to_vec()),
  }
}
