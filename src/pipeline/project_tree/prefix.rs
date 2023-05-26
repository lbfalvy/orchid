use std::rc::Rc;

use super::collect_ops::ExportedOpsCache;
use crate::ast::{Constant, Rule};
use crate::interner::{Interner, Tok};
use crate::representations::sourcefile::{FileEntry, Member, Namespace};
use crate::utils::Substack;

fn member_rec(
  // level
  mod_stack: Substack<Tok<String>>,
  // object
  data: Member,
  // context
  path: &[Tok<String>],
  ops_cache: &ExportedOpsCache,
  i: &Interner,
) -> Member {
  // let except = |op| imported.contains(&op);
  let except = |_| false;
  let prefix_v = path
    .iter()
    .copied()
    .chain(mod_stack.iter().rev_vec_clone().into_iter())
    .collect::<Vec<_>>();
  let prefix = i.i(&prefix_v);
  match data {
    Member::Namespace(Namespace { name, body }) => {
      let new_body = entv_rec(mod_stack.push(name), body, path, ops_cache, i);
      Member::Namespace(Namespace { name, body: new_body })
    },
    Member::Constant(constant) => Member::Constant(Constant {
      name: constant.name,
      value: constant.value.prefix(prefix, i, &except),
    }),
    Member::Rule(rule) => Member::Rule(Rule {
      prio: rule.prio,
      pattern: Rc::new(
        rule.pattern.iter().map(|e| e.prefix(prefix, i, &except)).collect(),
      ),
      template: Rc::new(
        rule.template.iter().map(|e| e.prefix(prefix, i, &except)).collect(),
      ),
    }),
  }
}

fn entv_rec(
  // level
  mod_stack: Substack<Tok<String>>,
  // object
  data: Vec<FileEntry>,
  // context
  path: &[Tok<String>],
  ops_cache: &ExportedOpsCache,
  i: &Interner,
) -> Vec<FileEntry> {
  data
    .into_iter()
    .map(|fe| match fe {
      FileEntry::Exported(mem) =>
        FileEntry::Exported(member_rec(mod_stack, mem, path, ops_cache, i)),
      FileEntry::Internal(mem) =>
        FileEntry::Internal(member_rec(mod_stack, mem, path, ops_cache, i)),
      // XXX should [FileEntry::Export] be prefixed?
      any => any,
    })
    .collect()
}

pub fn prefix(
  data: Vec<FileEntry>,
  path: &[Tok<String>],
  ops_cache: &ExportedOpsCache,
  i: &Interner,
) -> Vec<FileEntry> {
  entv_rec(Substack::Bottom, data, path, ops_cache, i)
}
