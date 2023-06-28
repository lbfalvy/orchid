use super::{ErrorPosition, ProjectError};
use crate::representations::project::ProjectModule;
#[allow(unused)] // For doc
use crate::tree::Module;
use crate::tree::WalkError;
use crate::utils::iter::box_once;
use crate::utils::BoxedIter;
use crate::{Interner, NameLike, Tok};

/// Error produced when an import refers to a nonexistent module
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NotFound {
  /// The file containing the invalid import
  pub file: Vec<String>,
  /// The invalid import path
  pub subpath: Vec<String>,
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
    prefix: &[Tok<String>],
    path: &[Tok<String>],
    orig: &ProjectModule<impl NameLike>,
    e: WalkError,
    i: &Interner,
  ) -> Self {
    let last_mod =
      orig.walk_ref(&path[..e.pos], false).expect("error occured on next step");
    let mut whole_path =
      prefix.iter().chain(path.iter()).map(|t| i.r(*t)).cloned();
    if let Some(file) = &last_mod.extra.file {
      Self {
        file: whole_path.by_ref().take(file.len()).collect(),
        subpath: whole_path.collect(),
      }
    } else {
      Self { file: whole_path.collect(), subpath: Vec::new() }
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
      self.subpath.join("::"),
      self.file.join("/"),
    )
  }
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    box_once(ErrorPosition::just_file(self.file.clone()))
  }
}
