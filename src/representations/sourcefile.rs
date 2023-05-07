use itertools::{Itertools, Either};

use crate::interner::{Token, Interner};
use crate::utils::BoxedIter;
use crate::ast::{Rule, Constant};


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Import {
  pub path: Token<Vec<Token<String>>>,
  /// If name is None, this is a wildcard import
  pub name: Option<Token<String>>
}
impl Import {
  /// Get the preload target space for this import - the prefix below
  /// which all files should be included in the compilation
  /// 
  /// Returns the path if this is a glob import, or the path plus the
  /// name if this is a specific import
  pub fn nonglob_path(&self, i: &Interner) -> Vec<Token<String>> {
    let mut path_vec = i.r(self.path).clone();
    if let Some(n) = self.name {
      path_vec.push(n)
    }
    path_vec
  }
}

/// Things that may be prefixed with an export
#[derive(Debug, Clone)]
pub enum Member {
  Rule(Rule),
  Constant(Constant),
  Namespace(Token<String>, Vec<FileEntry>)
}

/// Anything we might encounter in a file
#[derive(Debug, Clone)]
pub enum FileEntry {
  Import(Vec<Import>),
  Comment(String),
  Exported(Member),
  Internal(Member),
  Export(Vec<Token<String>>),
}

/// Summarize all imports from a file in a single list of qualified names 
pub fn imports<'a>(
  src: impl Iterator<Item = &'a FileEntry> + 'a
) -> impl Iterator<Item = &'a Import> + 'a {
  src.filter_map(|ent| match ent {
    FileEntry::Import(impv) => Some(impv.iter()),
    _ => None
  }).flatten()
}

/// Join the various redeclarations of namespaces.
/// Error if they're inconsistently exported
pub fn normalize_namespaces(
  src: BoxedIter<FileEntry>, i: &Interner
) -> Result<Vec<FileEntry>, Vec<Token<String>>> {
  let (mut namespaces, mut rest) = src
    .partition_map::<Vec<_>, Vec<_>, _, _, _>(|ent| match ent {
      FileEntry::Exported(Member::Namespace(name, body))
        => Either::Left((true, name, body)),
      FileEntry::Internal(Member::Namespace(name, body))
        => Either::Left((false, name, body)),
      other => Either::Right(other)
    });
  // Combine namespace blocks with the same name
  namespaces.sort_unstable_by_key(|(_, name, _)| *name);
  let mut lumped = namespaces.into_iter()
    .group_by(|(_, name, _)| *name).into_iter()
    .map(|(name, grp)| {
      let mut any_exported = false;
      let mut any_internal = false;
      let grp_src = grp.into_iter()
        .map(|(exported, name, body)| {
          if exported {any_exported = true}
          else {any_internal = true};
          (name, body) // Impure map is less than ideal but works
        })
        .flat_map(|(_, entv)| entv.into_iter());
      // Apply the function to the contents of these blocks too
      let data = normalize_namespaces(Box::new(grp_src), i)
        .map_err(|mut e| { e.push(name); e })?;
      let member = Member::Namespace(name, data);
      match (any_exported, any_internal) {
        (true, true) => Err(vec![name]),
        (true, false) => Ok(FileEntry::Exported(member)),
        (false, true) => Ok(FileEntry::Internal(member)),
        (false, false) => unreachable!("The group cannot be empty")
      }
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
  abs_location: &[Token<String>],
  rel_path: &[Token<String>],
  i: &Interner,
  is_child: &impl Fn(Token<String>) -> bool,
) -> Result<Vec<Token<String>>, ()> {
  let (head, tail) = if let Some(p) = rel_path.split_first() {p}
  else {return Ok(vec![])};
  if *head == i.i("super") {
    let (_, new_abs) = abs_location.split_last().ok_or(())?;
    if tail.len() == 0 {Ok(new_abs.to_vec())}
    else {absolute_path(new_abs, tail, i, is_child)}
  } else if *head == i.i("self") {
    Ok(abs_location.iter()
      .chain(tail.iter())
      .copied()
      .collect()
    )
  } else {
    Ok(rel_path.to_vec())
  }
}