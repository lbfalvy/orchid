use intern_all::{i, Tok};

use crate::name::{VName, VPath};
use crate::pipeline::project::{ItemKind, ProjectMemberRef, ProjectMod};
use crate::tree::ModMember;
use crate::utils::boxed_iter::{box_chain, BoxedIter};
use crate::utils::unwrap_or::unwrap_or;

pub struct WalkReport<'a> {
  pub target: ProjectMemberRef<'a>,
  pub abs_path: VName,
}

pub struct LinkWalkError<'a> {
  /// The last known valid path
  pub abs_path: Vec<Tok<String>>,
  /// The name that wasn't found
  pub name: Tok<String>,
  /// Leftover steps
  pub tail: BoxedIter<'a, Tok<String>>,
}
impl<'a> LinkWalkError<'a> {
  pub fn consumed_path(self) -> VName { VPath::new(self.abs_path).name_with_prefix(self.name) }
}

fn walk_with_links_rec<'a: 'b, 'b>(
  mut abs_path: Vec<Tok<String>>,
  root: &'a ProjectMod,
  cur: &'a ProjectMod,
  prev_tgt: ProjectMemberRef<'a>,
  mut path: impl Iterator<Item = Tok<String>> + 'b,
) -> Result<WalkReport<'a>, LinkWalkError<'b>> {
  let name = match path.next() {
    Some(sup) if sup == i!(str: "super") => {
      if abs_path.pop().is_none() {
        return Err(LinkWalkError { abs_path, name: sup, tail: Box::new(path) });
      }
      let path_acc = Vec::with_capacity(abs_path.len());
      let new_path = box_chain!(abs_path.into_iter(), path);
      let tgt = ProjectMemberRef::Mod(root);
      return walk_with_links_rec(path_acc, root, root, tgt, new_path);
    },
    Some(sup) if sup == i!(str: "self") => {
      let tgt = ProjectMemberRef::Mod(cur);
      return walk_with_links_rec(abs_path, root, cur, tgt, path);
    },
    Some(name) => name,
    // ends on this module
    None => {
      let abs_path = VName::new(abs_path).expect("Aliases are never empty");
      return Ok(WalkReport { target: prev_tgt, abs_path });
    },
  };
  let entry = unwrap_or! {cur.entries.get(&name); {
    // leads into a missing branch
    return Err(LinkWalkError{ abs_path, name, tail: Box::new(path) })
  }};
  match &entry.member {
    ModMember::Sub(m) => {
      // leads into submodule
      abs_path.push(name);
      let tgt = ProjectMemberRef::Mod(m);
      walk_with_links_rec(abs_path, root, m, tgt, path)
    },
    ModMember::Item(item) => match &item.kind {
      ItemKind::Alias(alias) => {
        // leads into alias (reset acc, cur, cur_entry)
        let path_acc = Vec::with_capacity(alias.len());
        let new_path = box_chain!(alias.iter(), path);
        let tgt = ProjectMemberRef::Mod(root);
        walk_with_links_rec(path_acc, root, root, tgt, new_path)
      },
      ItemKind::Const(_) | ItemKind::None => {
        abs_path.push(name);
        match path.next() {
          Some(name) => {
            // leads into leaf
            let tail = Box::new(path);
            Err(LinkWalkError { abs_path, name, tail })
          },
          None => {
            // ends on leaf
            let target = ProjectMemberRef::Item(item);
            let abs_path = VName::new(abs_path).expect("pushed just above");
            Ok(WalkReport { target, abs_path })
          },
        }
      },
    },
  }
}

/// Execute a walk down the tree, following aliases.
/// If the path ends on an alias, that alias is also resolved.
/// If the path leads out of the tree, the shortest failing path is returned
pub fn walk_with_links<'a: 'b, 'b>(
  root: &'a ProjectMod,
  path: impl Iterator<Item = Tok<String>> + 'b,
) -> Result<WalkReport<'a>, LinkWalkError<'b>> {
  let path_acc = path.size_hint().1.map_or_else(Vec::new, Vec::with_capacity);
  let tgt = ProjectMemberRef::Mod(root);
  let mut result = walk_with_links_rec(path_acc, root, root, tgt, path);
  // cut off excess preallocated space within normal vector growth policy
  let abs_path = match &mut result {
    Ok(rep) => rep.abs_path.vec_mut(),
    Err(err) => &mut err.abs_path,
  };
  abs_path.shrink_to(abs_path.len().next_power_of_two());
  result
}
