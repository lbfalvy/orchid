use std::iter;
use std::path::Path;

use hashbrown::HashMap;

use super::system::{IntoSystem, System};
use super::PreMacro;
use crate::error::ProjectResult;
use crate::pipeline::file_loader;
use crate::sourcefile::FileEntry;
use crate::utils::never;
use crate::{
  from_const_tree, parse_layer, vname_to_sym_tree, Interner, ProjectTree, Stok,
  VName,
};

/// A compiled environment ready to load user code. It stores the list of
/// systems and combines with usercode to produce a [Process]
pub struct Environment<'a> {
  /// [Interner] pseudo-global
  pub i: &'a Interner,
  systems: Vec<System<'a>>,
}
impl<'a> Environment<'a> {
  /// Initialize a new environment
  #[must_use]
  pub fn new(i: &'a Interner) -> Self { Self { i, systems: Vec::new() } }

  /// Register a new system in the environment
  #[must_use]
  pub fn add_system<'b: 'a>(mut self, is: impl IntoSystem<'b> + 'b) -> Self {
    self.systems.push(Box::new(is).into_system(self.i));
    self
  }

  /// Compile the environment from the set of systems and return it directly.
  /// See [#load_dir]
  pub fn compile(self) -> ProjectResult<CompiledEnv<'a>> {
    let Self { i, systems, .. } = self;
    let mut tree = from_const_tree(HashMap::new(), &[i.i("none")]);
    for sys in systems.iter() {
      let system_tree = from_const_tree(sys.constants.clone(), &sys.vname(i));
      tree = ProjectTree(never::unwrap_always(tree.0.overlay(system_tree.0)));
    }
    let mut prelude = vec![];
    for sys in systems.iter() {
      if !sys.code.is_empty() {
        tree = parse_layer(
          sys.code.keys().map(|sym| &sym[..]),
          &|k| sys.load_file(k),
          &tree,
          &prelude,
          i,
        )?;
      }
      prelude.extend_from_slice(&sys.prelude);
    }
    Ok(CompiledEnv { prelude, tree, systems })
  }

  /// Load a directory from the local file system as an Orchid project.
  pub fn load_dir(
    self,
    dir: &Path,
    target: &[Stok],
  ) -> ProjectResult<PreMacro<'a>> {
    let i = self.i;
    let CompiledEnv { prelude, systems, tree } = self.compile()?;
    let file_cache = file_loader::mk_dir_cache(dir.to_path_buf());
    let vname_tree = parse_layer(
      iter::once(target),
      &|path| file_cache.find(path),
      &tree,
      &prelude,
      i,
    )?;
    let tree = vname_to_sym_tree(vname_tree, i);
    PreMacro::new(tree, systems, i)
  }
}

/// Compiled environment waiting for usercode. An intermediate step between
/// [Environment] and [Process]
pub struct CompiledEnv<'a> {
  /// Namespace tree for pre-defined symbols with symbols at the leaves and
  /// rules defined on the nodes
  pub tree: ProjectTree<VName>,
  /// Lines prepended to each usercode file
  pub prelude: Vec<FileEntry>,
  /// List of systems to source handlers for the interpreter
  pub systems: Vec<System<'a>>,
}
