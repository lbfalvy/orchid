use hashbrown::HashMap;

use crate::error::{ErrorPosition, ProjectError};
use crate::interpreter::HandlerTable;
use crate::pipeline::file_loader::{IOResult, Loaded};
use crate::sourcefile::FileEntry;
use crate::utils::boxed_iter::box_empty;
use crate::utils::BoxedIter;
use crate::{ConstTree, Interner, Tok, VName};

/// A description of every point where an external library can hook into Orchid.
/// Intuitively, this can be thought of as a plugin
pub struct System<'a> {
  /// An identifier for the system used eg. in error reporting.
  pub name: Vec<String>,
  /// External functions and other constant values defined in AST form
  pub constants: HashMap<Tok<String>, ConstTree>,
  /// Orchid libraries defined by this system
  pub code: HashMap<VName, Loaded>,
  /// Prelude lines to be added to **subsequent** systems and usercode to
  /// expose the functionality of this system. The prelude is not added during
  /// the loading of this system
  pub prelude: Vec<FileEntry>,
  /// Handlers for actions defined in this system
  pub handlers: HandlerTable<'a>,
}
impl<'a> System<'a> {
  /// Intern the name of the system so that it can be used as an Orchid
  /// namespace
  #[must_use]
  pub fn vname(&self, i: &Interner) -> VName {
    self.name.iter().map(|s| i.i(s)).collect::<Vec<_>>()
  }

  /// Load a file from the system
  pub fn load_file(&self, path: &[Tok<String>]) -> IOResult {
    (self.code.get(path)).cloned().ok_or_else(|| {
      let err =
        MissingSystemCode { path: path.to_vec(), system: self.name.clone() };
      err.rc()
    })
  }
}

/// An error raised when a system fails to load a path. This usually means that
/// another system the current one depends on did not get loaded
#[derive(Debug)]
pub struct MissingSystemCode {
  path: VName,
  system: Vec<String>,
}
impl ProjectError for MissingSystemCode {
  fn description(&self) -> &str {
    "A system tried to import a path that doesn't exist"
  }
  fn message(&self) -> String {
    format!(
      "Path {} is not defined by {} or any system before it",
      Interner::extern_all(&self.path).join("::"),
      self.system.join("::")
    )
  }
  fn positions(&self) -> BoxedIter<ErrorPosition> { box_empty() }
}

/// Trait for objects that can be converted into a [System] in the presence
/// of an [Interner].
pub trait IntoSystem<'a> {
  /// Convert this object into a system using an interner
  fn into_system(self, i: &Interner) -> System<'a>;
}
