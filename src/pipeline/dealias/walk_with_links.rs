#[allow(unused)] // for doc
use crate::representations::project::ProjectEntry;
use crate::representations::project::{ItemKind, ProjectItem, ProjectMod};
use crate::tree::ModMember;
use crate::utils::{unwrap_or, BoxedIter};
use crate::{Interner, NameLike, Tok, VName};

/// The destination of a linked walk. [ProjectEntry] cannot be used for this
/// purpose because it might be the project root.
pub enum Target<'a, N: NameLike> {
  Mod(&'a ProjectMod<N>),
  Leaf(&'a ProjectItem<N>),
}

pub struct WalkReport<'a, N: NameLike> {
  pub target: Target<'a, N>,
  pub abs_path: VName,
  pub aliased: bool,
}

pub struct LinkWalkError<'a> {
  /// The last known valid path
  pub abs_path: VName,
  /// The name that wasn't found
  pub name: Tok<String>,
  /// Leftover steps
  pub tail: BoxedIter<'a, Tok<String>>,
  /// Whether an alias was ever encountered
  pub aliased: bool,
}

fn walk_with_links_rec<'a, 'b, N: NameLike>(
  mut abs_path: VName,
  root: &'a ProjectMod<N>,
  cur: &'a ProjectMod<N>,
  prev_tgt: Target<'a, N>,
  aliased: bool,
  mut path: impl Iterator<Item = Tok<String>> + 'b,
  l: bool,
) -> Result<WalkReport<'a, N>, LinkWalkError<'b>> {
  let name = unwrap_or! {path.next();
    // ends on this module
    return Ok(WalkReport{ target: prev_tgt, abs_path, aliased })
  };
  if l {
    eprintln!(
      "Resolving {} in {}",
      name,
      Interner::extern_all(&abs_path).join("::")
    )
  }
  let entry = unwrap_or! {cur.entries.get(&name); {
    // panic!("No entry {name} on {}", Interner::extern_all(&cur.extra.path).join("::"));
    // leads into a missing branch
    return Err(LinkWalkError{ abs_path, aliased, name, tail: Box::new(path) })
  }};
  match &entry.member {
    ModMember::Sub(m) => {
      // leads into submodule
      abs_path.push(name);
      walk_with_links_rec(abs_path, root, m, Target::Mod(m), aliased, path, l)
    },
    ModMember::Item(item) => match &item.kind {
      ItemKind::Alias(alias) => {
        // leads into alias (reset acc, cur, cur_entry)
        if l {
          eprintln!(
            "{} points to {}",
            Interner::extern_all(&abs_path).join("::"),
            Interner::extern_all(alias).join("::")
          )
        }
        abs_path.clone_from(alias);
        abs_path.extend(path);
        let path_acc = Vec::with_capacity(abs_path.len());
        let new_path = abs_path.into_iter();
        let tgt = Target::Mod(root);
        walk_with_links_rec(path_acc, root, root, tgt, true, new_path, l)
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
            let target = Target::Leaf(item);
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
pub fn walk_with_links<'a, N: NameLike>(
  root: &ProjectMod<N>,
  path: impl Iterator<Item = Tok<String>> + 'a,
  l: bool,
) -> Result<WalkReport<'_, N>, LinkWalkError<'a>> {
  let path_acc = path.size_hint().1.map_or_else(Vec::new, Vec::with_capacity);
  let mut result = walk_with_links_rec(
    path_acc,
    root,
    root,
    Target::Mod(root),
    false,
    path,
    l,
  );
  // cut off excess preallocated space within normal vector growth policy
  let abs_path = match &mut result {
    Ok(rep) => &mut rep.abs_path,
    Err(err) => &mut err.abs_path,
  };
  abs_path.shrink_to(abs_path.len().next_power_of_two());
  result
}
