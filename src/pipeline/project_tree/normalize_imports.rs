use super::collect_ops::ExportedOpsCache;
use crate::interner::{Interner, Tok};
use crate::pipeline::import_abs_path::import_abs_path;
use crate::representations::sourcefile::{
  FileEntry, Import, Member, Namespace,
};
use crate::representations::tree::{ModMember, Module};
use crate::utils::iter::box_once;
use crate::utils::{unwrap_or, BoxedIter, Substack};

fn member_rec(
  // level
  mod_stack: Substack<Tok<String>>,
  preparsed: &Module<impl Clone, impl Clone>,
  // object
  member: Member,
  // context
  path: &[Tok<String>],
  ops_cache: &ExportedOpsCache,
  i: &Interner,
) -> Member {
  match member {
    Member::Namespace(Namespace { name, body }) => {
      let subprep = unwrap_or!(
        &preparsed.items[&name].member => ModMember::Sub;
        unreachable!("This name must point to a namespace")
      );
      let new_body =
        entv_rec(mod_stack.push(name), subprep, body, path, ops_cache, i);
      Member::Namespace(Namespace { name, body: new_body })
    },
    any => any,
  }
}

fn entv_rec(
  // level
  mod_stack: Substack<Tok<String>>,
  preparsed: &Module<impl Clone, impl Clone>,
  // object
  data: Vec<FileEntry>,
  // context
  mod_path: &[Tok<String>],
  ops_cache: &ExportedOpsCache,
  i: &Interner,
) -> Vec<FileEntry> {
  data
    .into_iter()
    .map(|ent| match ent {
      FileEntry::Import(imps) => FileEntry::Import(
        imps
          .into_iter()
          .flat_map(|import| {
            if let Import { name: None, path } = import {
              let p = import_abs_path(mod_path, mod_stack, &i.r(path)[..], i)
                .expect("Should have emerged in preparsing");
              let names = ops_cache
                .find(&i.i(&p))
                .expect("Should have emerged in second parsing");
              let imports = names
                .iter()
                .map(move |&n| Import { name: Some(n), path })
                .collect::<Vec<_>>();
              Box::new(imports.into_iter()) as BoxedIter<Import>
            } else {
              box_once(import)
            }
          })
          .collect(),
      ),
      FileEntry::Exported(mem) => FileEntry::Exported(member_rec(
        mod_stack, preparsed, mem, mod_path, ops_cache, i,
      )),
      FileEntry::Internal(mem) => FileEntry::Internal(member_rec(
        mod_stack, preparsed, mem, mod_path, ops_cache, i,
      )),
      any => any,
    })
    .collect()
}

pub fn normalize_imports(
  preparsed: &Module<impl Clone, impl Clone>,
  data: Vec<FileEntry>,
  path: &[Tok<String>],
  ops_cache: &ExportedOpsCache,
  i: &Interner,
) -> Vec<FileEntry> {
  entv_rec(Substack::Bottom, preparsed, data, path, ops_cache, i)
}
