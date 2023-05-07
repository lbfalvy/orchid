use std::rc::Rc;

use crate::ast::{Constant, Rule};
use crate::interner::{Token, Interner};
use crate::utils::Substack;
use crate::representations::sourcefile::{Member, FileEntry};

use super::collect_ops::ExportedOpsCache;

fn member_rec(
  // level
  mod_stack: Substack<Token<String>>,
  // object
  data: Member,
  // context
  path: &[Token<String>],
  ops_cache: &ExportedOpsCache,
  i: &Interner
) -> Member {
  // let except = |op| imported.contains(&op);
  let except = |_| false;
  let prefix_v = path.iter().copied()
    .chain(mod_stack.iter().rev_vec_clone().into_iter())
    .collect::<Vec<_>>();
  let prefix = i.i(&prefix_v);
  match data {
    Member::Namespace(name, body) => {
      let new_body = entv_rec(
        mod_stack.push(name),
        body,
        path, ops_cache, i
      );
      Member::Namespace(name, new_body)
    }
    Member::Constant(constant) => Member::Constant(Constant{
      name: constant.name,
      value: constant.value.prefix(prefix, i, &except)
    }),
    Member::Rule(rule) => Member::Rule(Rule{
      prio: rule.prio,
      source: Rc::new(rule.source.iter()
        .map(|e| e.prefix(prefix, i, &except))
        .collect()
      ),
      target: Rc::new(rule.target.iter()
        .map(|e| e.prefix(prefix, i, &except))
        .collect()
      ),
    })
  }
}

fn entv_rec(
  // level
  mod_stack: Substack<Token<String>>,
  // object
  data: Vec<FileEntry>,
  // context
  path: &[Token<String>],
  ops_cache: &ExportedOpsCache,
  i: &Interner
) -> Vec<FileEntry> {
  data.into_iter().map(|fe| match fe {
    FileEntry::Exported(mem) => FileEntry::Exported(member_rec(
      mod_stack, mem, path, ops_cache, i
    )),
    FileEntry::Internal(mem) => FileEntry::Internal(member_rec(
      mod_stack, mem, path, ops_cache, i
    )),
    // XXX should [FileEntry::Export] be prefixed?
    any => any
  }).collect()
}

pub fn prefix(
  data: Vec<FileEntry>,
  path: &[Token<String>],
  ops_cache: &ExportedOpsCache,
  i: &Interner
) -> Vec<FileEntry> {
  entv_rec(Substack::Bottom, data, path, ops_cache, i)
}