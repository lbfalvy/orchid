use super::collect_ops::ExportedOpsCache;
use crate::ast::{Constant, Rule};
use crate::interner::{Interner, Tok};
use crate::representations::sourcefile::{FileEntry, Member, ModuleBlock};
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
  let prefix = (path.iter())
    .copied()
    .chain(mod_stack.iter().rev_vec_clone().into_iter())
    .collect::<Vec<_>>();
  match data {
    Member::Module(ModuleBlock { name, body }) => {
      let new_body = entv_rec(mod_stack.push(name), body, path, ops_cache, i);
      Member::Module(ModuleBlock { name, body: new_body })
    },
    Member::Constant(constant) => Member::Constant(Constant {
      name: constant.name,
      value: constant.value.prefix(&prefix, &|_| false),
    }),
    Member::Rule(rule) => Member::Rule(Rule {
      prio: rule.prio,
      pattern: (rule.pattern.into_iter())
        .map(|e| e.prefix(&prefix, &|_| false))
        .collect(),
      template: (rule.template.into_iter())
        .map(|e| e.prefix(&prefix, &|_| false))
        .collect(),
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
