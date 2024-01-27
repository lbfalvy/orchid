use intern_all::Tok;

use crate::name::{VName, VPath};
use crate::pipeline::project::{ItemKind, ProjectMemberRef, ProjectMod};
use crate::tree::ModMember;
use crate::utils::boxed_iter::BoxedIter;
use crate::utils::unwrap_or::unwrap_or;

pub struct WalkReport<'a> {
  pub target: ProjectMemberRef<'a>,
  pub abs_path: VName,
  pub aliased: bool,
}

pub struct LinkWalkError<'a> {
  /// The last known valid path
  pub abs_path: Vec<Tok<String>>,
  /// The name that wasn't found
  pub name: Tok<String>,
  /// Leftover steps
  pub tail: BoxedIter<'a, Tok<String>>,
  /// Whether an alias was ever encountered
  pub aliased: bool,
}
impl<'a> LinkWalkError<'a> {
  pub fn consumed_path(self) -> VName {
    VPath::new(self.abs_path).as_prefix_of(self.name)
  }
}

fn walk_with_links_rec<'a, 'b>(
  mut abs_path: Vec<Tok<String>>,
  root: &'a ProjectMod,
  cur: &'a ProjectMod,
  prev_tgt: ProjectMemberRef<'a>,
  aliased: bool,
  mut path: impl Iterator<Item = Tok<String>> + 'b,
) -> Result<WalkReport<'a>, LinkWalkError<'b>> {
  let name = match path.next() {
    Some(name) => name,
    // ends on this module
    None => {
      let abs_path = VName::new(abs_path).expect("Aliases are never empty");
      return Ok(WalkReport { target: prev_tgt, abs_path, aliased });
    },
  };
  let entry = unwrap_or! {cur.entries.get(&name); {
    // leads into a missing branch
    return Err(LinkWalkError{ abs_path, aliased, name, tail: Box::new(path) })
  }};
  match &entry.member {
    ModMember::Sub(m) => {
      // leads into submodule
      abs_path.push(name);
      let tgt = ProjectMemberRef::Mod(m);
      walk_with_links_rec(abs_path, root, m, tgt, aliased, path)
    },
    ModMember::Item(item) => match &item.kind {
      ItemKind::Alias(alias) => {
        // leads into alias (reset acc, cur, cur_entry)
        abs_path.clear();
        abs_path.extend_from_slice(&alias[..]);
        abs_path.extend(path);
        let path_acc = Vec::with_capacity(abs_path.len());
        let new_path = abs_path.into_iter();
        let tgt = ProjectMemberRef::Mod(root);
        walk_with_links_rec(path_acc, root, root, tgt, true, new_path)
      },
      ItemKind::Const(_) | ItemKind::None => {
        abs_path.push(name);
        match path.next() {
          Some(name) => {
            // leads into leaf
            let tail = Box::new(path);
            Err(LinkWalkError { abs_path, aliased, name, tail })
          },
          None => {
            // ends on leaf
            let target = ProjectMemberRef::Item(item);
            let abs_path = VName::new(abs_path).expect("pushed just above");
            Ok(WalkReport { target, abs_path, aliased })
          },
        }
      },
    },
  }
}

/// Execute a walk down the tree, following aliases.
/// If the path ends on an alias, that alias is also resolved.
/// If the path leads out of the tree, the shortest failing path is returned
pub fn walk_with_links<'a>(
  root: &ProjectMod,
  path: impl Iterator<Item = Tok<String>> + 'a,
) -> Result<WalkReport<'_>, LinkWalkError<'a>> {
  let path_acc = path.size_hint().1.map_or_else(Vec::new, Vec::with_capacity);
  let tgt = ProjectMemberRef::Mod(root);
  let mut result = walk_with_links_rec(path_acc, root, root, tgt, false, path);
  // cut off excess preallocated space within normal vector growth policy
  let abs_path = match &mut result {
    Ok(rep) => rep.abs_path.vec_mut(),
    Err(err) => &mut err.abs_path,
  };
  abs_path.shrink_to(abs_path.len().next_power_of_two());
  result
}
