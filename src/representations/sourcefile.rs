use std::rc::Rc;
use std::collections::HashSet;

use lasso::Spur;

use crate::box_chain;
use crate::utils::{Stackframe, iter::box_empty};
use crate::ast::{Rule, Expr};


#[derive(Debug, Clone)]
pub struct Import {
  pub path: Rc<Vec<Spur>>,
  /// If name is None, this is a wildcard import
  pub name: Option<Spur>
}

/// Anything we might encounter in a file
#[derive(Clone)]
pub enum FileEntry {
  Import(Vec<Import>),
  Comment(String),
  /// The bool indicates whether the rule is exported, that is,
  /// whether tokens uniquely defined inside it should be exported
  Rule(Rule, bool),
  Export(Vec<Rc<Vec<Spur>>>),
  LazyModule(Spur)
}

/// Collect all names that occur in an expression
fn find_all_names_expr(
  expr: &Expr
) -> HashSet<Rc<Vec<Spur>>> {
  let mut ret = HashSet::new();
  expr.visit_names(
    Stackframe::new(Rc::default()),
    &mut |n| { ret.insert(n); }
  );
  ret
}

/// Collect all exported names (and a lot of other words) from a file
pub fn exported_names(
  src: &[FileEntry]
) -> HashSet<Rc<Vec<Spur>>> {
  src.iter().flat_map(|ent| match ent {
    FileEntry::Rule(Rule{source, target, ..}, true) =>
      box_chain!(source.iter(), target.iter()),
    _ => box_empty()
  }).flat_map(|e| find_all_names_expr(e))
  .chain(
    src.iter().filter_map(|ent| {
      if let FileEntry::Export(names) = ent {
        Some(names.iter())
      } else {None}
    }).flatten().cloned()
  ).chain(
    src.iter().filter_map(|ent| {
      if let FileEntry::LazyModule(lm) = ent {
        Some(Rc::new(vec![*lm]))
      } else {None}
    })
  ).collect()
}

/// Summarize all imports from a file in a single list of qualified names 
pub fn imports<'a, 'b, I>(
  src: I
) -> impl Iterator<Item = &'b Import> + 'a
where I: Iterator<Item = &'b FileEntry> + 'a {
  src.filter_map(|ent| match ent {
    FileEntry::Import(impv) => Some(impv.iter()),
    _ => None
  }).flatten()
}

