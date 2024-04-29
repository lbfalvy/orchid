//! The main structure of the façade, collects systems and exposes various
//! operations over the whole set.

use std::borrow::Borrow;
use std::path::PathBuf;

use intern_all::i;

use super::macro_runner::MacroRunner;
use super::merge_trees::merge_trees;
use super::process::Process;
use super::system::{IntoSystem, System};
use super::unbound_ref::validate_refs;
use crate::error::Reporter;
use crate::gen::tree::ConstTree;
use crate::interpreter::context::RunEnv;
use crate::interpreter::handler::HandlerTable;
use crate::location::{CodeGenInfo, CodeOrigin};
use crate::name::{PathSlice, Sym, VPath};
use crate::pipeline::load_project::{load_project, ProjectContext};
use crate::pipeline::project::ProjectTree;
use crate::sym;
use crate::utils::combine::Combine;
use crate::utils::sequence::Sequence;
use crate::virt_fs::{DeclTree, DirNode, Loaded, VirtFS};

/// A compiled environment ready to load user code. It stores the list of
/// systems and combines with usercode to produce a [Process]
pub struct Loader<'a> {
  systems: Vec<System<'a>>,
}
impl<'a> Loader<'a> {
  /// Initialize a new environment
  #[must_use]
  pub fn new() -> Self { Self { systems: Vec::new() } }

  /// Retrieve the list of systems
  pub fn systems(&self) -> impl Iterator<Item = &System<'a>> { self.systems.iter() }

  /// Register a new system in the environment
  #[must_use]
  pub fn add_system<'b: 'a>(mut self, is: impl IntoSystem<'b> + 'b) -> Self {
    self.systems.push(Box::new(is).into_system());
    self
  }

  /// Extract the systems from the environment
  pub fn into_systems(self) -> Vec<System<'a>> { self.systems }

  /// Initialize an environment with a prepared list of systems
  pub fn from_systems(sys: impl IntoIterator<Item = System<'a>>) -> Self {
    Self { systems: sys.into_iter().collect() }
  }

  /// Combine the `constants` fields of all systems
  pub fn constants(&self) -> ConstTree {
    (self.systems())
      .try_fold(ConstTree::tree::<&str>([]), |acc, sys| acc.combine(sys.constants.clone()))
      .expect("Conflicting const trees")
  }

  /// Extract the command handlers from the systems, consuming the loader in the
  /// process. This has to consume the systems because handler tables aren't
  /// Copy. It also establishes the practice that environments live on the
  /// stack.
  pub fn handlers(&self) -> HandlerTable<'_> {
    (self.systems.iter()).fold(HandlerTable::new(), |t, sys| t.link(&sys.handlers))
  }

  /// Compile the environment from the set of systems and return it directly.
  /// See [#load_dir]
  pub fn project_ctx<'b>(&self, reporter: &'b Reporter) -> ProjectContext<'_, 'b> {
    ProjectContext {
      lexer_plugins: Sequence::new(|| {
        self.systems().flat_map(|sys| &sys.lexer_plugins).map(|b| &**b)
      }),
      line_parsers: Sequence::new(|| {
        self.systems().flat_map(|sys| &sys.line_parsers).map(|b| &**b)
      }),
      preludes: Sequence::new(|| self.systems().flat_map(|sys| &sys.prelude)),
      reporter,
    }
  }

  /// Combine source code from all systems with the specified directory into a
  /// common [VirtFS]
  pub fn make_dir_fs(&self, dir: PathBuf) -> DeclTree {
    let dir_node = DirNode::new(dir, ".orc").rc();
    DeclTree::tree([("tree", DeclTree::leaf(dir_node))])
  }

  /// All system trees merged into one
  pub fn system_fs(&self) -> DeclTree {
    (self.systems().try_fold(DeclTree::empty(), |acc, sub| acc.combine(sub.code.clone())))
      .expect("Conflicting system trees")
  }

  /// A wrapper around [load_project] that only takes the arguments that aren't
  /// fully specified by systems
  pub fn load_project_main(
    &self,
    entrypoints: impl IntoIterator<Item = Sym>,
    root: DeclTree,
    reporter: &Reporter,
  ) -> ProjectTree {
    let tgt_loc = CodeOrigin::Gen(CodeGenInfo::no_details(sym!(facade::entrypoint)));
    let constants = self.constants().unwrap_mod();
    let targets = entrypoints.into_iter().map(|s| (s, tgt_loc.clone()));
    let root = self.system_fs().combine(root).expect("System trees conflict with root");
    load_project(&self.project_ctx(reporter), targets, &constants, &root)
  }

  /// A wrapper around [load_project] that only takes the arguments that aren't
  /// fully specified by systems
  pub fn load_project(&self, root: DeclTree, reporter: &Reporter) -> ProjectTree {
    let mut orc_files: Vec<VPath> = Vec::new();
    find_all_orc_files([].borrow(), &mut orc_files, &root);
    let entrypoints = (orc_files.into_iter()).map(|p| p.name_with_suffix(i!(str: "tree")).to_sym());
    let tgt_loc = CodeOrigin::Gen(CodeGenInfo::no_details(sym!(facade::entrypoint)));
    let constants = self.constants().unwrap_mod();
    let targets = entrypoints.into_iter().map(|s| (s, tgt_loc.clone()));
    let root = self.system_fs().combine(root).expect("System trees conflict with root");
    load_project(&self.project_ctx(reporter), targets, &constants, &root)
  }

  /// Load a directory from the local file system as an Orchid project.
  /// File loading proceeds along import statements and ignores all files
  /// not reachable from the specified file.
  pub fn load_main(
    &self,
    dir: PathBuf,
    targets: impl IntoIterator<Item = Sym>,
    reporter: &Reporter,
  ) -> ProjectTree {
    self.load_project_main(targets, self.make_dir_fs(dir), reporter)
  }

  /// Load every orchid file in a directory
  pub fn load_dir(&self, dir: PathBuf, reporter: &Reporter) -> ProjectTree {
    self.load_project(self.make_dir_fs(dir), reporter)
  }

  /// Build a process by calling other utilities in [crate::facade]. A sort of
  /// facade over the facade. If you need a custom file system, consider
  /// combining this with [Loader::load_project]. For usage with
  /// [Loader::load_main] and [Loader::load_dir] we offer the shorthands
  /// [Loader::proc_main] and [Loader::proc_dir].
  pub fn proc(
    &'a self,
    tree: ProjectTree,
    check_refs: bool,
    macro_limit: Option<usize>,
    reporter: &Reporter,
  ) -> Process<'a> {
    let mr = MacroRunner::new(&tree, macro_limit, reporter);
    let pm_tree = mr.run_macros(tree, reporter);
    let consts = merge_trees(pm_tree.all_consts(), self.systems(), reporter);
    if check_refs {
      validate_refs(consts.keys().cloned().collect(), reporter, &mut |sym, location| {
        (consts.get(&sym).map(|nc| nc.value.clone()))
          .ok_or_else(|| RunEnv::sym_not_found(sym, location))
      });
    }
    Process::new(consts, self.handlers())
  }

  /// Load a project and process everything
  pub fn proc_dir(
    &'a self,
    dir: PathBuf,
    check_refs: bool,
    macro_limit: Option<usize>,
    reporter: &Reporter,
  ) -> Process<'a> {
    self.proc(self.load_dir(dir.to_owned(), reporter), check_refs, macro_limit, reporter)
  }

  /// Load a project and process everything to load specific symbols
  pub fn proc_main(
    &'a self,
    dir: PathBuf,
    targets: impl IntoIterator<Item = Sym>,
    check_refs: bool,
    macro_limit: Option<usize>,
    reporter: &Reporter,
  ) -> Process<'a> {
    self.proc(self.load_main(dir.to_owned(), targets, reporter), check_refs, macro_limit, reporter)
  }
}

impl<'a> Default for Loader<'a> {
  fn default() -> Self { Self::new() }
}

fn find_all_orc_files(path: &PathSlice, paths: &mut Vec<VPath>, vfs: &impl VirtFS) {
  match vfs.read(path) {
    Err(_) => (),
    Ok(Loaded::Code(_)) => paths.push(path.to_vpath()),
    Ok(Loaded::Collection(items)) => items
      .iter()
      .for_each(|suffix| find_all_orc_files(&path.to_vpath().suffix([suffix.clone()]), paths, vfs)),
  }
}
