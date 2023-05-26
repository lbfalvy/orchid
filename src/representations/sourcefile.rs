//! Building blocks of a source file
use itertools::{Either, Itertools};

use crate::ast::{Constant, Rule};
use crate::interner::{Interner, Sym, Tok};
use crate::utils::{unwrap_or, BoxedIter};

/// An import pointing at another module, either specifying the symbol to be
/// imported or importing all available symbols with a globstar (*)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Import {
  pub path: Sym,
  /// If name is None, this is a wildcard import
  pub name: Option<Tok<String>>,
}
impl Import {
  /// Get the preload target space for this import - the prefix below
  /// which all files should be included in the compilation
  ///
  /// Returns the path if this is a glob import, or the path plus the
  /// name if this is a specific import
  pub fn nonglob_path(&self, i: &Interner) -> Vec<Tok<String>> {
    let mut path_vec = i.r(self.path).clone();
    if let Some(n) = self.name {
      path_vec.push(n)
    }
    path_vec
  }
}

/// A namespace block
#[derive(Debug, Clone)]
pub struct Namespace {
  pub name: Tok<String>,
  pub body: Vec<FileEntry>,
}

/// Things that may be prefixed with an export
#[derive(Debug, Clone)]
pub enum Member {
  Rule(Rule),
  Constant(Constant),
  Namespace(Namespace),
}

/// Anything we might encounter in a file
#[derive(Debug, Clone)]
pub enum FileEntry {
  Import(Vec<Import>),
  Comment(String),
  Exported(Member),
  Internal(Member),
  Export(Vec<Tok<String>>),
}

/// Summarize all imports from a file in a single list of qualified names
pub fn imports<'a>(
  src: impl Iterator<Item = &'a FileEntry> + 'a,
) -> impl Iterator<Item = &'a Import> + 'a {
  src
    .filter_map(|ent| match ent {
      FileEntry::Import(impv) => Some(impv.iter()),
      _ => None,
    })
    .flatten()
}

/// Join the various redeclarations of namespaces.
/// Error if they're inconsistently exported
pub fn normalize_namespaces(
  src: BoxedIter<FileEntry>,
) -> Result<Vec<FileEntry>, Vec<Tok<String>>> {
  let (mut namespaces, mut rest) = src
    .partition_map::<Vec<_>, Vec<_>, _, _, _>(|ent| match ent {
      FileEntry::Exported(Member::Namespace(ns)) => Either::Left((true, ns)),
      FileEntry::Internal(Member::Namespace(ns)) => Either::Left((false, ns)),
      other => Either::Right(other),
    });
  // Combine namespace blocks with the same name
  namespaces.sort_unstable_by_key(|(_, ns)| ns.name);
  let mut lumped = namespaces
    .into_iter()
    .group_by(|(_, ns)| ns.name)
    .into_iter()
    .map(|(name, grp)| {
      let mut any_exported = false;
      let mut any_internal = false;
      let grp_src = grp
        .into_iter()
        .map(|(exported, ns)| {
          if exported {
            any_exported = true
          } else {
            any_internal = true
          };
          ns // Impure map is less than ideal but works
        })
        .flat_map(|ns| ns.body.into_iter());
      // Apply the function to the contents of these blocks too
      let body = normalize_namespaces(Box::new(grp_src)).map_err(|mut e| {
        e.push(name);
        e
      })?;
      let member = Member::Namespace(Namespace { name, body });
      match (any_exported, any_internal) {
        (true, true) => Err(vec![name]),
        (true, false) => Ok(FileEntry::Exported(member)),
        (false, true) => Ok(FileEntry::Internal(member)),
        (false, false) => unreachable!("The group cannot be empty"),
      }
    })
    .collect::<Result<Vec<_>, _>>()?;
  rest.append(&mut lumped);
  Ok(rest)
}

/// Produced by [absolute_path] if there are more `super` segments in the
/// import than the length of the current absolute path
#[derive(Debug, Clone)]
pub struct TooManySupers;

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
) -> Result<Vec<Tok<String>>, TooManySupers> {
  let (head, tail) = unwrap_or!(rel_path.split_first();
    return Ok(vec![])
  );
  if *head == i.i("super") {
    let (_, new_abs) = abs_location.split_last().ok_or(TooManySupers)?;
    if tail.is_empty() {
      Ok(new_abs.to_vec())
    } else {
      absolute_path(new_abs, tail, i)
    }
  } else if *head == i.i("self") {
    Ok(abs_location.iter().chain(tail.iter()).copied().collect())
  } else {
    Ok(rel_path.to_vec())
  }
}
