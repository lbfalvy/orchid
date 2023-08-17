use crate::interner::Tok;
use crate::representations::sourcefile::{FileEntry, Member, ModuleBlock};

fn member_rec(
  // object
  member: Member,
  // context
  path: &[Tok<String>],
  prelude: &[FileEntry],
) -> Member {
  match member {
    Member::Module(ModuleBlock { name, body }) => {
      let new_body = entv_rec(body, path, prelude);
      Member::Module(ModuleBlock { name, body: new_body })
    },
    any => any,
  }
}

fn entv_rec(
  // object
  data: Vec<FileEntry>,
  // context
  mod_path: &[Tok<String>],
  prelude: &[FileEntry],
) -> Vec<FileEntry> {
  prelude
    .iter()
    .cloned()
    .chain(data.into_iter().map(|ent| match ent {
      FileEntry::Exported(mem) =>
        FileEntry::Exported(member_rec(mem, mod_path, prelude)),
      FileEntry::Internal(mem) =>
        FileEntry::Internal(member_rec(mem, mod_path, prelude)),
      any => any,
    }))
    .collect()
}

pub fn add_prelude(
  data: Vec<FileEntry>,
  path: &[Tok<String>],
  prelude: &[FileEntry],
) -> Vec<FileEntry> {
  entv_rec(data, path, prelude)
}
