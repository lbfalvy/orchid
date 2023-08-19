use std::rc::Rc;

use super::ProjectError;
use crate::representations::project::ProjectModule;
#[allow(unused)] // For doc
use crate::tree::Module;
use crate::tree::WalkError;
use crate::{Interner, Location, NameLike, Tok, VName};

/// Error produced when an import refers to a nonexistent module
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NotFound {
  /// The module that imported the invalid path
  pub source: Option<VName>,
  /// The file not containing the expected path
  pub file: VName,
  /// The invalid import path
  pub subpath: VName,
}
impl NotFound {
  /// Produce this error from the parameters of [Module]`::walk_ref` and a
  /// [WalkError]
  ///
  /// # Panics
  ///
  /// - if `path` is shorter than the `pos` of the error
  /// - if a walk up to but not including `pos` fails
  ///
  /// Basically, if `e` was not produced by the `walk*` methods called on
  /// `path`.
  pub fn from_walk_error(
    source: &[Tok<String>],
    prefix: &[Tok<String>],
    path: &[Tok<String>],
    orig: &ProjectModule<impl NameLike>,
    e: WalkError,
  ) -> Self {
    let last_mod =
      orig.walk_ref(&path[..e.pos], false).expect("error occured on next step");
    let mut whole_path = prefix.iter().chain(path.iter()).cloned();
    if let Some(file) = &last_mod.extra.file {
      Self {
        source: Some(source.to_vec()),
        file: whole_path.by_ref().take(file.len()).collect(),
        subpath: whole_path.collect(),
      }
    } else {
      Self {
        source: Some(source.to_vec()),
        file: whole_path.collect(),
        subpath: Vec::new(),
      }
    }
  }
}
impl ProjectError for NotFound {
  fn description(&self) -> &str {
    "an import refers to a nonexistent module"
  }
  fn message(&self) -> String {
    format!(
      "module {} in {} was not found",
      Interner::extern_all(&self.subpath).join("::"),
      Interner::extern_all(&self.file).join("/"),
    )
  }
  fn one_position(&self) -> crate::Location {
    Location::File(Rc::new(Interner::extern_all(&self.file)))
  }
}
