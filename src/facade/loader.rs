use std::path::{Path, PathBuf};
use std::{fs, iter};

use intern_all::{i, Tok};
use substack::Substack;

use super::system::{IntoSystem, System};
use crate::error::ProjectResult;
use crate::gen::tree::ConstTree;
use crate::interpreter::handler::HandlerTable;
use crate::location::{CodeGenInfo, CodeLocation};
use crate::name::{Sym, VPath};
use crate::pipeline::load_solution::{load_solution, SolutionContext};
use crate::pipeline::project::ProjectTree;
use crate::utils::combine::Combine;
use crate::utils::sequence::Sequence;
use crate::utils::unwrap_or::unwrap_or;
use crate::virt_fs::{DeclTree, DirNode, VirtFS};

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
  pub fn systems(&self) -> impl Iterator<Item = &System<'a>> {
    self.systems.iter()
  }

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
      .try_fold(ConstTree::tree::<&str>([]), |acc, sys| {
        acc.combine(sys.constants.clone())
      })
      .expect("Conflicting const trees")
  }

  pub fn handlers(self) -> HandlerTable<'a> {
    (self.systems.into_iter())
      .fold(HandlerTable::new(), |t, sys| t.combine(sys.handlers))
  }

  /// Compile the environment from the set of systems and return it directly.
  /// See [#load_dir]
  pub fn solution_ctx(&self) -> ProjectResult<SolutionContext> {
    Ok(SolutionContext {
      lexer_plugins: Sequence::new(|| {
        self.systems().flat_map(|sys| &sys.lexer_plugins).map(|b| &**b)
      }),
      line_parsers: Sequence::new(|| {
        self.systems().flat_map(|sys| &sys.line_parsers).map(|b| &**b)
      }),
      preludes: Sequence::new(|| self.systems().flat_map(|sys| &sys.prelude)),
    })
  }

  /// Combine source code from all systems with the specified directory into a
  /// common [VirtFS]
  pub fn make_dir_tree(&self, dir: PathBuf) -> DeclTree {
    let dir_node = DirNode::new(dir, ".orc").rc();
    let base = DeclTree::tree([("tree", DeclTree::leaf(dir_node))]);
    (self.systems().try_fold(base, |acc, sub| acc.combine(sub.code.clone())))
      .expect("Conflicting system trees")
  }

  /// Load a directory from the local file system as an Orchid project.
  /// File loading proceeds along import statements and ignores all files
  /// not reachable from the specified file.
  pub fn load_main(
    &self,
    dir: PathBuf,
    target: Sym,
  ) -> ProjectResult<ProjectTree> {
    let ctx = self.solution_ctx()?;
    let tgt_loc =
      CodeLocation::Gen(CodeGenInfo::no_details("facade::entrypoint"));
    let root = self.make_dir_tree(dir.clone());
    let targets = iter::once((target, tgt_loc));
    let constants = self.constants().unwrap_mod();
    load_solution(ctx, targets, &constants, &root)
  }

  /// Load every orchid file in a directory
  pub fn load_dir(&self, dir: PathBuf) -> ProjectResult<ProjectTree> {
    let ctx = self.solution_ctx()?;
    let tgt_loc =
      CodeLocation::Gen(CodeGenInfo::no_details("facade::entrypoint"));
    let mut orc_files: Vec<VPath> = Vec::new();
    find_all_orc_files(&dir, &mut orc_files, Substack::Bottom);
    let root = self.make_dir_tree(dir.clone());
    let constants = self.constants().unwrap_mod();
    let targets = (orc_files.into_iter())
      .map(|p| (p.as_suffix_of(i("tree")).to_sym(), tgt_loc.clone()));
    load_solution(ctx, targets, &constants, &root)
  }
}

impl<'a> Default for Loader<'a> {
  fn default() -> Self { Self::new() }
}

fn find_all_orc_files(
  path: &Path,
  paths: &mut Vec<VPath>,
  stack: Substack<'_, Tok<String>>,
) {
  assert!(path.exists(), "find_all_orc_files encountered missing path");
  if path.is_symlink() {
    let path = unwrap_or!(fs::read_link(path).ok(); return);
    find_all_orc_files(&path, paths, stack)
  } else if path.is_file() {
    if path.extension().and_then(|t| t.to_str()) == Some("orc") {
      paths.push(VPath(stack.unreverse()))
    }
  } else if path.is_dir() {
    let entries = unwrap_or!(path.read_dir().ok(); return);
    for entry in entries.filter_map(Result::ok) {
      let name = unwrap_or!(entry.file_name().into_string().ok(); return);
      find_all_orc_files(&entry.path(), paths, stack.push(i(&name)))
    }
  }
}
