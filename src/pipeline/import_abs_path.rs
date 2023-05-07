use std::rc::Rc;

use crate::representations::tree::Module;
use crate::representations::sourcefile::absolute_path;
use crate::utils::{Substack};
use crate::interner::{Token, Interner};

use super::error::{ProjectError, TooManySupers};

pub fn import_abs_path(
  src_path: &[Token<String>],
  mod_stack: Substack<Token<String>>,
  module: &Module<impl Clone, impl Clone>,
  import_path: &[Token<String>],
  i: &Interner,
) -> Result<Vec<Token<String>>, Rc<dyn ProjectError>> {
  // path of module within file
  let mod_pathv = mod_stack.iter().rev_vec_clone();
  // path of module within compilation
  let abs_pathv = src_path.iter().copied()
    .chain(mod_pathv.iter().copied())
    .collect::<Vec<_>>();
  // preload-target path relative to module
  // preload-target path within compilation
  absolute_path(&abs_pathv, import_path, i, &|n| {
    module.items.contains_key(&n)
  }).map_err(|_| TooManySupers{
      path: import_path.iter().map(|t| i.r(*t)).cloned().collect(),
      offender_file: src_path.iter().map(|t| i.r(*t)).cloned().collect(),
      offender_mod: mod_pathv.iter().map(|t| i.r(*t)).cloned().collect(),
  }.rc())
}