use std::rc::Rc;

use itertools::Itertools;

use super::{ErrorPosition, ProjectError};
use crate::parse::Entry;
use crate::utils::BoxedIter;

/// Produced by stages that parse text when it fails.
pub struct ParseErrorWithTokens {
  /// The complete source of the faulty file
  pub full_source: String,
  /// Tokens, if the error did not occur during tokenization
  pub tokens: Vec<Entry>,
  /// The parse error produced by Chumsky
  pub error: Rc<dyn ProjectError>,
}
impl ProjectError for ParseErrorWithTokens {
  fn description(&self) -> &str {
    self.error.description()
  }
  fn message(&self) -> String {
    format!(
      "Failed to parse code: {}\nTokenized source for context:\n{}",
      self.error.message(),
      self.tokens.iter().map(|t| t.to_string()).join(" "),
    )
  }
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    self.error.positions()
  }
}
