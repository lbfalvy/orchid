use std::rc::Rc;

use crate::representations::location::Location;
use crate::utils::BoxedIter;
use crate::parse::ParseError;

use super::ErrorPosition;
use super::ProjectError;

/// Produced by stages that parse text when it fails.
#[derive(Debug)]
pub struct ParseErrorWithPath {
  pub full_source: String,
  pub path: Vec<String>,
  pub error: ParseError
}
impl ProjectError for ParseErrorWithPath {
  fn description(&self) -> &str {"Failed to parse code"}
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    match &self.error {
      ParseError::Lex(lex) => Box::new(lex.iter().map(|s| ErrorPosition {
        location: Location::Range {
          file: Rc::new(self.path.clone()),
          range: s.span(),
        },
        message: Some(s.to_string())
      })),
      ParseError::Ast(ast) => Box::new(ast.iter().map(|(_i, s)| ErrorPosition {
        location: s.found().map(|e| Location::Range {
          file: Rc::new(self.path.clone()),
          range: e.range.clone()
        }).unwrap_or_else(|| Location::File(Rc::new(self.path.clone()))),
        message: Some(s.label().unwrap_or("Parse error").to_string())
      })),
    }
  }
}