//! Unified extension struct instances of which are catalogued by
//! [super::loader::Loader]. Language extensions must implement [IntoSystem].

use crate::error::{ErrorPosition, ProjectError};
use crate::gen::tree::ConstTree;
use crate::interpreter::handler::HandlerTable;
use crate::name::VName;
use crate::parse::lex_plugin::LexerPlugin;
use crate::parse::parse_plugin::ParseLinePlugin;
use crate::pipeline::load_project::Prelude;
use crate::virt_fs::DeclTree;

/// A description of every point where an external library can hook into Orchid.
/// Intuitively, this can be thought of as a plugin
pub struct System<'a> {
  /// An identifier for the system used eg. in error reporting.
  pub name: &'a str,
  /// External functions and other constant values defined in AST form
  pub constants: ConstTree,
  /// Orchid libraries defined by this system
  pub code: DeclTree,
  /// Prelude lines to be added to the head of files to expose the
  /// functionality of this system. A glob import from the first path is
  /// added to every file outside the prefix specified by the second path
  pub prelude: Vec<Prelude>,
  /// Handlers for actions defined in this system
  pub handlers: HandlerTable<'a>,
  /// Custom lexer for the source code representation atomic data.
  /// These take priority over builtin lexers so the syntax they
  /// match should be unambiguous
  pub lexer_plugins: Vec<Box<dyn LexerPlugin + 'a>>,
  /// Parser that processes custom line types into their representation in the
  /// module tree
  pub line_parsers: Vec<Box<dyn ParseLinePlugin>>,
}
impl<'a> System<'a> {
  /// Intern the name of the system so that it can be used as an Orchid
  /// namespace
  #[must_use]
  pub fn vname(&self) -> VName {
    VName::parse(self.name).expect("Systems must have a non-empty name")
  }
}

/// An error raised when a system fails to load a path. This usually means that
/// another system the current one depends on did not get loaded
#[derive(Debug)]
pub struct MissingSystemCode {
  path: VName,
  system: Vec<String>,
  referrer: VName,
}
impl ProjectError for MissingSystemCode {
  const DESCRIPTION: &'static str = "A system tried to import a path that doesn't exist";
  fn message(&self) -> String {
    format!(
      "Path {} imported by {} is not defined by {} or any system before it",
      self.path,
      self.referrer,
      self.system.join("::")
    )
  }
  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> { [] }
}

/// Trait for objects that can be converted into a [System].
pub trait IntoSystem<'a> {
  /// Convert this object into a system using an interner
  fn into_system(self) -> System<'a>;
}
