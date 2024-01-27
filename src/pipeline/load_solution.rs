//! Load an Orchid project by starting from one or more entry points and
//! following the imports

use std::collections::VecDeque;
use std::sync::Arc;

use hashbrown::{HashMap, HashSet};
use intern_all::{sweep_t, Tok};

use super::dealias::resolve_aliases::resolve_aliases;
use super::process_source::{process_ns, resolve_globs, GlobImports};
use super::project::{ItemKind, ProjItem, ProjXEnt, ProjectMod, ProjectTree};
use crate::error::{
  bundle_location, ErrorPosition, ProjectError, ProjectResult,
};
use crate::location::{CodeGenInfo, CodeLocation, SourceCode, SourceRange};
use crate::name::{PathSlice, Sym, VName, VPath};
use crate::parse::context::ParseCtxImpl;
use crate::parse::facade::parse_file;
use crate::parse::lex_plugin::LexerPlugin;
use crate::parse::parse_plugin::ParseLinePlugin;
use crate::tree::{ModEntry, ModMember, Module};
use crate::utils::combine::Combine;
use crate::utils::sequence::Sequence;
use crate::virt_fs::{Loaded, VirtFS};

// apply layer:
// 1. build trees
// Question: what if a file is not found?
// - raising an error would risk failing on a std module
// - moving on could obscure very simple errors
// can we get rid of layers and show system sources alongside user sources?
// what would break? Can we break it?
// the project moves into a prefix, imports are either super:: or user::
// custom support for root:: specifier
// virtual file tree is back on
// systems get free reign on their subtree, less jank
// would also solve some weird accidental private member aliasing issues

/// Split off the longest prefix accepted by the validator
fn split_max_prefix<'a, T>(
  path: &'a [T],
  is_valid: &impl Fn(&[T]) -> bool,
) -> Option<(&'a [T], &'a [T])> {
  (0..=path.len())
    .rev()
    .map(|i| path.split_at(i))
    .find(|(file, _)| is_valid(file))
}

/// Represents a prelude / implicit import requested by a library.
/// A prelude extends any module with a glob import from the target module
/// unless its path begins with exclude.
#[derive(Debug, Clone)]
pub struct Prelude {
  /// Path the glob imports will point to
  pub target: VName,
  /// subtree to exclude (typically the region the prelude collates items from)
  pub exclude: VName,
  /// Location data attached to the aliases
  pub owner: CodeGenInfo,
}

/// Hooks and extensions to the source loading process
#[derive(Clone)]
pub struct SolutionContext<'a> {
  /// Callbacks from the lexer to support literals of custom datatypes
  pub lexer_plugins: Sequence<'a, &'a (dyn LexerPlugin + 'a)>,
  /// Callbacks from the parser to support custom module tree elements
  pub line_parsers: Sequence<'a, &'a (dyn ParseLinePlugin + 'a)>,
  /// Lines prepended to various modules to import "global" values
  pub preludes: Sequence<'a, &'a Prelude>,
}
impl<'a> SolutionContext<'a> {
  /// Derive context for the parser
  pub fn parsing(&self, code: SourceCode) -> ParseCtxImpl<'a> {
    ParseCtxImpl {
      code,
      lexers: self.lexer_plugins.clone(),
      line_parsers: self.line_parsers.clone(),
    }
  }
}

/// Load source files from a source tree and parse them starting from the
/// specified targets and following imports. An in-memory environment tree is
/// used to allow imports from modules that are defined by other loading steps
/// and later merged into this source code.
pub fn load_solution(
  ctx: SolutionContext,
  targets: impl IntoIterator<Item = (Sym, CodeLocation)>,
  env: &Module<impl Sized, impl Sized, impl Sized>,
  fs: &impl VirtFS,
) -> ProjectResult<ProjectTree> {
  let mut target_queue = VecDeque::<(Sym, CodeLocation)>::new();
  target_queue.extend(targets.into_iter());
  target_queue.extend(
    (ctx.preludes.iter())
      .map(|p| (p.target.to_sym(), CodeLocation::Gen(p.owner.clone()))),
  );
  let mut known_files = HashSet::new();
  let mut tree_acc: ProjectMod = Module::wrap([]);
  let mut glob_acc: GlobImports = Module::wrap([]);
  while let Some((target, referrer)) = target_queue.pop_front() {
    let path_parts = split_max_prefix(&target[..], &|p| {
      fs.read(PathSlice(p)).map(|l| l.is_code()).unwrap_or(false)
    });
    if let Some((filename, _)) = path_parts {
      if known_files.contains(filename) {
        continue;
      }
      known_files.insert(filename.to_vec());
      let path = VPath(filename.to_vec());
      let loaded = fs
        .read(PathSlice(filename))
        .map_err(|e| bundle_location(&referrer, &*e))?;
      let code = match loaded {
        Loaded::Collection(_) =>
          return Err(UnexpectedDirectory { path }.pack()),
        Loaded::Code(source) => SourceCode { source, path: Arc::new(path) },
      };
      let full_range =
        SourceRange { range: 0..code.source.len(), code: code.clone() };
      let lines = parse_file(&ctx.parsing(code.clone()))?;
      let report = process_ns(code.path, lines, full_range)?;
      target_queue.extend(
        (report.external_references.into_iter())
          .map(|(k, v)| (k, CodeLocation::Source(v))),
      );
      if !report.comments.is_empty() && filename.is_empty() {
        todo!("panic - module op comments on root are lost")
      }
      let mut comments = Some(report.comments);
      let mut module = report.module;
      let mut glob = report.glob_imports;
      for i in (0..filename.len()).rev() {
        // i over valid indices of filename
        let key = filename[i].clone(); // last segment
        let comments = comments.take().into_iter().flatten().collect();
        glob =
          Module::wrap([(key.clone(), ModEntry::wrap(ModMember::Sub(glob)))]);
        module = Module::wrap([(key, ModEntry {
          member: ModMember::Sub(module),
          x: ProjXEnt { comments, ..Default::default() },
        })]);
      }
      glob_acc = (glob_acc.combine(glob))
        .expect("source code loaded for two nested paths");
      tree_acc = (tree_acc.combine(module))
        .expect("source code loaded for two nested paths");
    } else {
      known_files.insert(target[..].to_vec());
      // If the path is not within a file, load it as directory
      match fs.read(target.as_path_slice()) {
        Ok(Loaded::Collection(c)) => target_queue
          .extend(c.iter().map(|e| (Sym::parse(e).unwrap(), referrer.clone()))),
        Ok(Loaded::Code(_)) => unreachable!("Should have split to self and []"),
        // Ignore error if the path is walkable in the const tree
        Err(_) if env.walk1_ref(&[], &target[..], |_| true).is_ok() => (),
        Err(e) => return Err(bundle_location(&referrer, &*e)),
      }
    }
  }
  let mut contention = HashMap::new();
  resolve_globs(
    VPath(vec![]),
    glob_acc,
    ctx.preludes.clone(),
    &mut tree_acc,
    env,
    &mut contention,
  )?;
  let ret = resolve_aliases(tree_acc, env)?;
  for ((glob, original), locations) in contention {
    let (glob_val, _) = ret
      .walk1_ref(&[], &glob[..], |_| true)
      .expect("Should've emerged in dealias");
    let (original_val, _) = ret
      .walk1_ref(&[], &original[..], |_| true)
      .expect("Should've emerged in dealias");
    let glob_real = match &glob_val.member {
      ModMember::Item(ProjItem { kind: ItemKind::Alias(glob_tgt) }) => glob_tgt,
      _ => &glob,
    };
    let original_real = match &original_val.member {
      ModMember::Item(ProjItem { kind: ItemKind::Alias(orig_tgt) }) => orig_tgt,
      _ => &original,
    };
    if glob_real != original_real {
      let real = original_real.clone();
      let glob_real = glob_real.clone();
      let err = ConflictingGlobs { real, glob_real, original, glob, locations };
      return Err(err.pack());
    }
  }
  sweep_t::<String>();
  sweep_t::<Vec<Tok<String>>>();
  Ok(ProjectTree(ret))
}

/// Produced when a stage that deals specifically with code encounters
/// a path that refers to a directory
#[derive(Debug)]
struct UnexpectedDirectory {
  /// Path to the offending collection
  pub path: VPath,
}
impl ProjectError for UnexpectedDirectory {
  const DESCRIPTION: &'static str = "A stage that deals specifically with code \
    encountered a path that refers to a directory";
  fn message(&self) -> String {
    format!("{} was expected to be a file", self.path)
  }
  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> { [] }
}

#[derive(Debug)]
struct ConflictingGlobs {
  original: Sym,
  real: Sym,
  glob: Sym,
  glob_real: Sym,
  locations: Vec<CodeLocation>,
}
impl ProjectError for ConflictingGlobs {
  const DESCRIPTION: &'static str =
    "A symbol from a glob import conflicts with an existing name";
  fn message(&self) -> String {
    let Self { glob, glob_real, original, real, .. } = self;
    format!(
      "glob import included {glob} which resolved to {glob_real}. \
      This conflicts with {original} which resolved to {real}"
    )
  }
  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> {
    (self.locations.iter())
      .map(|l| ErrorPosition { location: l.clone(), message: None })
  }
}
