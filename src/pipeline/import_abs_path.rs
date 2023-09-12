use crate::error::ProjectResult;
use crate::interner::{Interner, Tok};
use crate::representations::sourcefile::absolute_path;
use crate::utils::Substack;
use crate::{Location, VName};

pub fn import_abs_path(
  src_path: &[Tok<String>],
  mod_stack: Substack<Tok<String>>,
  import_path: &[Tok<String>],
  i: &Interner,
  location: &Location,
) -> ProjectResult<VName> {
  // path of module within file
  let mod_pathv = mod_stack.iter().rev_vec_clone();
  // path of module within compilation
  let abs_pathv =
    (src_path.iter()).chain(mod_pathv.iter()).cloned().collect::<Vec<_>>();
  // preload-target path relative to module
  // preload-target path within compilation
  absolute_path(&abs_pathv, import_path, i, location)
}
