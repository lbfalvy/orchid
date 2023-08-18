use std::rc::Rc;

use super::ProjectError;
use crate::{Interner, Location, VName};

/// Produced when a stage that deals specifically with code encounters
/// a path that refers to a directory
#[derive(Debug)]
pub struct UnexpectedDirectory {
  /// Path to the offending collection
  pub path: VName,
}
impl ProjectError for UnexpectedDirectory {
  fn description(&self) -> &str {
    "A stage that deals specifically with code encountered a path that refers \
     to a directory"
  }
  fn one_position(&self, i: &Interner) -> crate::Location {
    Location::File(Rc::new(i.extern_all(&self.path)))
  }

  fn message(&self, i: &Interner) -> String {
    format!(
      "{} was expected to be a file but a directory was found",
      i.extern_all(&self.path).join("/")
    )
  }
}
