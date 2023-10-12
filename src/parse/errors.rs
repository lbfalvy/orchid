use std::rc::Rc;

use itertools::Itertools;

use super::{Entry, Lexeme};
use crate::error::ProjectError;
use crate::{Location, Tok};

#[derive(Debug)]
pub struct LineNeedsPrefix {
  pub entry: Entry,
}
impl ProjectError for LineNeedsPrefix {
  fn description(&self) -> &str { "This linetype requires a prefix" }
  fn one_position(&self) -> Location { self.entry.location() }
  fn message(&self) -> String {
    format!("{} cannot appear at the beginning of a line", self.entry)
  }
}

#[derive(Debug)]
pub struct UnexpectedEOL {
  /// Last entry before EOL
  pub entry: Entry,
}
impl ProjectError for UnexpectedEOL {
  fn description(&self) -> &str { "The line ended abruptly" }
  fn one_position(&self) -> Location { self.entry.location() }
  fn message(&self) -> String {
    "The line ends unexpectedly here. In Orchid, all line breaks outside \
     parentheses start a new declaration"
      .to_string()
  }
}

pub struct ExpectedEOL {
  pub location: Location,
}
impl ProjectError for ExpectedEOL {
  fn description(&self) -> &str { "Expected the end of the line" }
  fn one_position(&self) -> Location { self.location.clone() }
}

#[derive(Debug)]
pub struct ExpectedName {
  pub entry: Entry,
}
impl ExpectedName {
  pub fn expect(entry: &Entry) -> Result<Tok<String>, Rc<dyn ProjectError>> {
    match &entry.lexeme {
      Lexeme::Name(n) => Ok(n.clone()),
      _ => Err(Self { entry: entry.clone() }.rc()),
    }
  }
}
impl ProjectError for ExpectedName {
  fn description(&self) -> &str { "A name was expected" }
  fn one_position(&self) -> Location { self.entry.location() }
  fn message(&self) -> String {
    if self.entry.is_keyword() {
      format!(
        "{} is a restricted keyword and cannot be used as a name",
        self.entry
      )
    } else {
      format!("Expected a name, found {}", self.entry)
    }
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
    if e.lexeme.strict_eq(&l) {
      return Ok(());
    }
    Err(Self { expected: vec![l], or_name: false, found: e.clone() }.rc())
  }
}
impl ProjectError for Expected {
  fn description(&self) -> &str { "A concrete token was expected" }
  fn one_position(&self) -> Location { self.found.location() }
  fn message(&self) -> String {
    let list = match &self.expected[..] {
      &[] => return "Unsatisfiable expectation".to_string(),
      [only] => only.to_string(),
      [a, b] => format!("either {a} or {b}"),
      [variants @ .., last] => {
        format!("any of {} or {last}", variants.iter().join(", "))
      },
    };
    let or_name = if self.or_name { " or a name" } else { "" };
    format!("Expected {list}{or_name} but found {}", self.found)
  }
}

pub struct ReservedToken {
  pub entry: Entry,
}
impl ProjectError for ReservedToken {
  fn description(&self) -> &str { "Syntax reserved for future use" }
  fn one_position(&self) -> Location { self.entry.location() }
  fn message(&self) -> String { format!("{} is a reserved token", self.entry) }
}

pub struct BadTokenInRegion {
  pub entry: Entry,
  pub region: &'static str,
}
impl ProjectError for BadTokenInRegion {
  fn description(&self) -> &str { "An unexpected token was found" }
  fn one_position(&self) -> Location { self.entry.location() }
  fn message(&self) -> String {
    format!("{} cannot appear in {}", self.entry, self.region)
  }
}

pub struct NotFound {
  pub expected: &'static str,
  pub location: Location,
}
impl ProjectError for NotFound {
  fn description(&self) -> &str { "A specific lexeme was expected" }
  fn one_position(&self) -> Location { self.location.clone() }
  fn message(&self) -> String { format!("{} was expected", self.expected) }
}

pub struct LeadingNS(pub Location);
impl ProjectError for LeadingNS {
  fn description(&self) -> &str { ":: can only follow a name token" }
  fn one_position(&self) -> Location { self.0.clone() }
}

pub struct MisalignedParen(pub Entry);
impl ProjectError for MisalignedParen {
  fn description(&self) -> &str { "(), [] and {} must always pair up" }
  fn one_position(&self) -> Location { self.0.location() }
  fn message(&self) -> String { format!("This {} has no pair", self.0) }
}

pub struct NamespacedExport(pub Location);
impl ProjectError for NamespacedExport {
  fn description(&self) -> &str { "Only local names may be exported" }
  fn one_position(&self) -> Location { self.0.clone() }
}

pub struct GlobExport(pub Location);
impl ProjectError for GlobExport {
  fn description(&self) -> &str { "Globstars are not allowed in exports" }
  fn one_position(&self) -> Location { self.0.clone() }
}

pub struct NoStringEnd(pub Location);
impl ProjectError for NoStringEnd {
  fn description(&self) -> &str { "A string literal was not closed with `\"`" }
  fn one_position(&self) -> Location { self.0.clone() }
}

pub struct NoCommentEnd(pub Location);
impl ProjectError for NoCommentEnd {
  fn description(&self) -> &str { "a comment was not closed with `]--`" }
  fn one_position(&self) -> Location { self.0.clone() }
}

pub struct FloatPlacehPrio(pub Location);
impl ProjectError for FloatPlacehPrio {
  fn description(&self) -> &str {
    "a placeholder priority has a decimal point or a negative exponent"
  }
  fn one_position(&self) -> Location { self.0.clone() }
}

pub struct NaNLiteral(pub Location);
impl ProjectError for NaNLiteral {
  fn description(&self) -> &str { "float literal decoded to NaN" }
  fn one_position(&self) -> Location { self.0.clone() }
}

pub struct LiteralOverflow(pub Location);
impl ProjectError for LiteralOverflow {
  fn description(&self) -> &str {
    "number literal described number greater than usize::MAX"
  }
  fn one_position(&self) -> Location { self.0.clone() }
}

pub struct ExpectedDigit(pub Location);
impl ProjectError for ExpectedDigit {
  fn description(&self) -> &str { "expected a digit" }
  fn one_position(&self) -> Location { self.0.clone() }
}

pub struct NotHex(pub Location);
impl ProjectError for NotHex {
  fn description(&self) -> &str { "Expected a hex digit" }
  fn one_position(&self) -> Location { self.0.clone() }
}

pub struct BadCodePoint(pub Location);
impl ProjectError for BadCodePoint {
  fn description(&self) -> &str {
    "\\uXXXX escape sequence does not describe valid code point"
  }
  fn one_position(&self) -> Location { self.0.clone() }
}

pub struct BadEscapeSequence(pub Location);
impl ProjectError for BadEscapeSequence {
  fn description(&self) -> &str { "Unrecognized escape sequence" }
  fn one_position(&self) -> Location { self.0.clone() }
}
