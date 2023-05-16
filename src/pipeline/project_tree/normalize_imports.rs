use crate::representations::tree::{Module, ModMember};
use crate::representations::sourcefile::{Member, FileEntry, Import};
use crate::utils::BoxedIter;
use crate::utils::{Substack, iter::box_once};
use crate::interner::{Interner, Token};
use crate::pipeline::import_abs_path::import_abs_path;

use super::collect_ops::ExportedOpsCache;

fn member_rec(
  // level
  mod_stack: Substack<Token<String>>,
  preparsed: &Module<impl Clone, impl Clone>,
  // object
  member: Member,
  // context
  path: &[Token<String>],
  ops_cache: &ExportedOpsCache,
  i: &Interner
) -> Member {
  match member {
    Member::Namespace(name, body) => {
      let prepmember = &preparsed.items[&name].member;
      let subprep = if let ModMember::Sub(m) = prepmember {m.clone()}
      else {unreachable!("This name must point to a namespace")};
      let new_body = entv_rec(
        mod_stack.push(name),
        subprep.as_ref(),
        body,
        path, ops_cache, i
      );
      Member::Namespace(name, new_body)
    },
    any => any
  }
}

fn entv_rec(
  // level
  mod_stack: Substack<Token<String>>,
  preparsed: &Module<impl Clone, impl Clone>,
  // object
  data: Vec<FileEntry>,
  // context
  mod_path: &[Token<String>],
  ops_cache: &ExportedOpsCache,
  i: &Interner
) -> Vec<FileEntry> {
  data.into_iter()
    .map(|ent| match ent {
      FileEntry::Import(imps) => FileEntry::Import(imps.into_iter()
        .flat_map(|import| if let Import{ name: None, path } = import {
          let p = import_abs_path(
            mod_path, mod_stack, preparsed, &i.r(path)[..], i
          ).expect("Should have emerged in preparsing");
          let names = ops_cache.find(&i.i(&p))
            .expect("Should have emerged in second parsing");
          let imports = names.iter()
            .map(move |&n| Import{ name: Some(n), path })
            .collect::<Vec<_>>();
          Box::new(imports.into_iter()) as BoxedIter<Import>
        } else {box_once(import)})
        .collect()
      ),
      FileEntry::Exported(mem) => FileEntry::Exported(member_rec(
        mod_stack, preparsed, mem, mod_path, ops_cache, i
      )),
      FileEntry::Internal(mem) => FileEntry::Internal(member_rec(
        mod_stack, preparsed, mem, mod_path, ops_cache, i
      )),
      any => any
    })
    .collect()
}

pub fn normalize_imports(
  preparsed: &Module<impl Clone, impl Clone>,
  data: Vec<FileEntry>,
  path: &[Token<String>],
  ops_cache: &ExportedOpsCache,
  i: &Interner
) -> Vec<FileEntry> {
  entv_rec(Substack::Bottom, preparsed, data, path, ops_cache, i)
}