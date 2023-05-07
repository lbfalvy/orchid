use crate::representations::sourcefile::{Member, FileEntry};
use crate::interner::Token;

fn member_rec(
  // object
  member: Member,
  // context
  path: &[Token<String>],
  prelude: &[FileEntry],
) -> Member {
  match member {
    Member::Namespace(name, body) => {
      let new_body = entv_rec(
        body,
        path,
        prelude
      );
      Member::Namespace(name, new_body)
    },
    any => any
  }
}

fn entv_rec(
  // object
  data: Vec<FileEntry>,
  // context
  mod_path: &[Token<String>],
  prelude: &[FileEntry],
) -> Vec<FileEntry> {
  prelude.iter().cloned()
    .chain(data.into_iter()
      .map(|ent| match ent {
        FileEntry::Exported(mem) => FileEntry::Exported(member_rec(
          mem, mod_path, prelude
        )),
        FileEntry::Internal(mem) => FileEntry::Internal(member_rec(
          mem, mod_path, prelude
        )),
        any => any
      })
    )
    .collect()
}

pub fn add_prelude(
  data: Vec<FileEntry>,
  path: &[Token<String>],
  prelude: &[FileEntry],
) -> Vec<FileEntry> {
  entv_rec(data, path, prelude)
}