use super::{ErrorPosition, ProjectError};
#[allow(unused)] // for doc
use crate::parse_layer;
use crate::utils::iter::box_empty;
use crate::utils::BoxedIter;

/// Error produced when [parse_layer] is called without targets. This function
/// produces an error instead of returning a straightforward empty tree because
/// the edge case of no targets is often an error and should generally be
/// handled explicitly
#[derive(Debug)]
pub struct NoTargets;

impl ProjectError for NoTargets {
  fn description(&self) -> &str {
    "No targets were specified for layer parsing"
  }

  fn positions(&self) -> BoxedIter<ErrorPosition> {
    box_empty()
  }
}
