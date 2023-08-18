use std::rc::Rc;

use chumsky::prelude::Simple;
use itertools::Itertools;

use super::{Entry, Lexeme};
use crate::error::{ErrorPosition, ProjectError};
use crate::interner::InternedDisplay;
use crate::utils::BoxedIter;
use crate::{Interner, Location, Tok};

#[derive(Debug)]
pub struct LineNeedsPrefix {
  pub entry: Entry,
}
impl ProjectError for LineNeedsPrefix {
  fn description(&self) -> &str {
    "This linetype requires a prefix"
  }
  fn message(&self, i: &Interner) -> String {
    format!("{} cannot appear at the beginning of a line", self.entry.bundle(i))
  }
  fn one_position(&self, _i: &Interner) -> Location {
    self.entry.location()
  }
}

#[derive(Debug)]
pub struct UnexpectedEOL {
  /// Last entry before EOL
  pub entry: Entry,
}
impl ProjectError for UnexpectedEOL {
  fn description(&self) -> &str {
    "The line ended abruptly"
  }

  fn message(&self, _i: &Interner) -> String {
    "The line ends unexpectedly here. In Orchid, all line breaks outside \
     parentheses start a new declaration"
      .to_string()
  }

  fn one_position(&self, _i: &Interner) -> Location {
    self.entry.location()
  }
}

pub struct ExpectedEOL {
  pub location: Location,
}
impl ProjectError for ExpectedEOL {
  fn description(&self) -> &str {
    "Expected the end of the line"
  }
  fn one_position(&self, _i: &Interner) -> Location {
    self.location.clone()
  }
}

#[derive(Debug)]
pub struct ExpectedName {
  pub entry: Entry,
}
impl ExpectedName {
  pub fn expect(entry: &Entry) -> Result<Tok<String>, Rc<dyn ProjectError>> {
    match entry.lexeme {
      Lexeme::Name(n) => Ok(n),
      _ => Err(Self { entry: entry.clone() }.rc()),
    }
  }
}
impl ProjectError for ExpectedName {
  fn description(&self) -> &str {
    "A name was expected here, but something else was found"
  }

  fn message(&self, i: &Interner) -> String {
    if self.entry.is_keyword() {
      format!(
        "{} is a restricted keyword and cannot be used as a name",
        self.entry.bundle(i)
      )
    } else {
      format!("Expected a name, found {}", self.entry.bundle(i))
    }
  }

  fn one_position(&self, _i: &Interner) -> Location {
    self.entry.location()
  }
}

#[derive()]
pub struct Expected {
  pub expected: Vec<Lexeme>,
  pub or_name: bool,
  pub found: Entry,
}
impl Expected {
  pub fn expect(l: Lexeme, e: &Entry) -> Result<(), Rc<dyn ProjectError>> {
    if e.lexeme != l {
      return Err(
        Self { expected: vec![l], or_name: false, found: e.clone() }.rc(),
      );
    }
    Ok(())
  }
}
impl ProjectError for Expected {
  fn description(&self) -> &str {
    "A concrete token was expected but something else was found"
  }
  fn message(&self, i: &Interner) -> String {
    let list = match &self.expected[..] {
      &[] => return "Unsatisfiable expectation".to_string(),
      [only] => only.to_string_i(i),
      [a, b] => format!("either {} or {}", a.bundle(i), b.bundle(i)),
      [variants @ .., last] => format!(
        "any of {} or {}",
        variants.iter().map(|l| l.to_string_i(i)).join(", "),
        last.bundle(i)
      ),
    };
    let or_name = if self.or_name { " or a name" } else { "" };
    format!("Expected {}{} but found {}", list, or_name, self.found.bundle(i))
  }

  fn one_position(&self, _i: &Interner) -> Location {
    self.found.location()
  }
}

pub struct ReservedToken {
  pub entry: Entry,
}
impl ProjectError for ReservedToken {
  fn description(&self) -> &str {
    "A token reserved for future use was found in the code"
  }

  fn message(&self, i: &Interner) -> String {
    format!("{} is a reserved token", self.entry.bundle(i))
  }

  fn one_position(&self, _i: &Interner) -> Location {
    self.entry.location()
  }
}

pub struct BadTokenInRegion {
  pub entry: Entry,
  pub region: &'static str,
}
impl ProjectError for BadTokenInRegion {
  fn description(&self) -> &str {
    "A token was found in a region where it should not appear"
  }

  fn message(&self, i: &Interner) -> String {
    format!("{} cannot appear in {}", self.entry.bundle(i), self.region)
  }

  fn one_position(&self, _i: &Interner) -> Location {
    self.entry.location()
  }
}

pub struct NotFound {
  pub expected: &'static str,
  pub location: Location,
}
impl ProjectError for NotFound {
  fn description(&self) -> &str {
    "A specific lexeme was expected but not found in the given range"
  }

  fn message(&self, _i: &Interner) -> String {
    format!("{} was expected", self.expected)
  }

  fn one_position(&self, _i: &Interner) -> Location {
    self.location.clone()
  }
}

pub struct LeadingNS {
  pub location: Location,
}
impl ProjectError for LeadingNS {
  fn description(&self) -> &str {
    ":: can only follow a name token"
  }
  fn one_position(&self, _i: &Interner) -> Location {
    self.location.clone()
  }
}

pub struct MisalignedParen {
  pub entry: Entry,
}
impl ProjectError for MisalignedParen {
  fn description(&self) -> &str {
    "Parentheses (), [] and {} must always pair up"
  }
  fn message(&self, i: &Interner) -> String {
    format!("This {} has no pair", self.entry.bundle(i))
  }
  fn one_position(&self, _i: &Interner) -> Location {
    self.entry.location()
  }
}

pub struct NamespacedExport {
  pub location: Location,
}
impl ProjectError for NamespacedExport {
  fn description(&self) -> &str {
    "Exports can only refer to unnamespaced names in the local namespace"
  }
  fn one_position(&self, _i: &Interner) -> Location {
    self.location.clone()
  }
}

pub struct GlobExport {
  pub location: Location,
}
impl ProjectError for GlobExport {
  fn description(&self) -> &str {
    "Exports can only refer to concrete names, globstars are not allowed"
  }
  fn one_position(&self, _i: &Interner) -> Location {
    self.location.clone()
  }
}

pub struct LexError {
  pub errors: Vec<Simple<char>>,
  pub file: Rc<Vec<String>>,
}
impl ProjectError for LexError {
  fn description(&self) -> &str {
    "An error occured during tokenization"
  }
  fn positions(&self, _i: &Interner) -> BoxedIter<ErrorPosition> {
    let file = self.file.clone();
    Box::new(self.errors.iter().map(move |s| ErrorPosition {
      location: Location::Range { file: file.clone(), range: s.span() },
      message: Some(format!("{}", s)),
    }))
  }
}
