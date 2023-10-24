//! Errors produced by the parser. Plugins are encouraged to reuse these where
//! applicable.

use std::rc::Rc;

use itertools::Itertools;

use super::{Entry, Lexeme, Stream};
use crate::ast::PType;
use crate::error::{ProjectError, ProjectResult};
use crate::{Location, Tok};

/// A line does not begin with an identifying keyword
#[derive(Debug)]
pub struct LineNeedsPrefix {
  /// Erroneous line starter
  pub entry: Entry,
}
impl ProjectError for LineNeedsPrefix {
  fn description(&self) -> &str { "This linetype requires a prefix" }
  fn one_position(&self) -> Location { self.entry.location() }
  fn message(&self) -> String {
    format!("{} cannot appear at the beginning of a line", self.entry)
  }
}

/// The line ends abruptly
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

/// The line should have ended
pub struct ExpectedEOL {
  /// Location of the last valid or first excessive token
  pub location: Location,
}
impl ProjectError for ExpectedEOL {
  fn description(&self) -> &str { "Expected the end of the line" }
  fn one_position(&self) -> Location { self.location.clone() }
}

/// A name was expected
#[derive(Debug)]
pub struct ExpectedName {
  /// Non-name entry
  pub entry: Entry,
}
impl ExpectedName {
  /// If the entry is a name, return its text. If it's not, produce this error.
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
    format!("Expected a name, found {}", self.entry)
  }
}

/// A specific lexeme was expected
#[derive()]
pub struct Expected {
  /// The lexemes that would have been acceptable
  pub expected: Vec<Lexeme>,
  /// Whether a name would also have been acceptable (multiname)
  pub or_name: bool,
  /// What was actually found
  pub found: Entry,
}
impl Expected {
  /// Assert that the entry contains exactly the specified lexeme
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

/// A token reserved for future use was found in the code
pub struct ReservedToken {
  /// The offending token
  pub entry: Entry,
}
impl ProjectError for ReservedToken {
  fn description(&self) -> &str { "Syntax reserved for future use" }
  fn one_position(&self) -> Location { self.entry.location() }
  fn message(&self) -> String { format!("{} is a reserved token", self.entry) }
}

/// A token was found where it doesn't belong
pub struct BadTokenInRegion {
  /// What was found
  pub entry: Entry,
  /// Human-readable name of the region where it should not appear
  pub region: &'static str,
}
impl ProjectError for BadTokenInRegion {
  fn description(&self) -> &str { "An unexpected token was found" }
  fn one_position(&self) -> Location { self.entry.location() }
  fn message(&self) -> String {
    format!("{} cannot appear in {}", self.entry, self.region)
  }
}

/// A specific lexeme was searched but not found
pub struct NotFound {
  /// Human-readable description of what was searched
  pub expected: &'static str,
  /// Area covered by the search
  pub location: Location,
}
impl ProjectError for NotFound {
  fn description(&self) -> &str { "A specific lexeme was expected" }
  fn one_position(&self) -> Location { self.location.clone() }
  fn message(&self) -> String { format!("{} was expected", self.expected) }
}

/// :: found on its own somewhere other than a general export
pub struct LeadingNS(pub Location);
impl ProjectError for LeadingNS {
  fn description(&self) -> &str { ":: can only follow a name token" }
  fn one_position(&self) -> Location { self.0.clone() }
}

/// Parens don't pair up
pub struct MisalignedParen(pub Entry);
impl ProjectError for MisalignedParen {
  fn description(&self) -> &str { "(), [] and {} must always pair up" }
  fn one_position(&self) -> Location { self.0.location() }
  fn message(&self) -> String { format!("This {} has no pair", self.0) }
}

/// Export line contains a complex name
pub struct NamespacedExport(pub Location);
impl ProjectError for NamespacedExport {
  fn description(&self) -> &str { "Only local names may be exported" }
  fn one_position(&self) -> Location { self.0.clone() }
}

/// Export line contains *
pub struct GlobExport(pub Location);
impl ProjectError for GlobExport {
  fn description(&self) -> &str { "Globstars are not allowed in exports" }
  fn one_position(&self) -> Location { self.0.clone() }
}

/// String literal never ends
pub struct NoStringEnd(pub Location);
impl ProjectError for NoStringEnd {
  fn description(&self) -> &str { "A string literal was not closed with `\"`" }
  fn one_position(&self) -> Location { self.0.clone() }
}

/// Comment never ends
pub struct NoCommentEnd(pub Location);
impl ProjectError for NoCommentEnd {
  fn description(&self) -> &str { "a comment was not closed with `]--`" }
  fn one_position(&self) -> Location { self.0.clone() }
}

/// A placeholder's priority is a floating point number
pub struct FloatPlacehPrio(pub Location);
impl ProjectError for FloatPlacehPrio {
  fn description(&self) -> &str {
    "a placeholder priority has a decimal point or a negative exponent"
  }
  fn one_position(&self) -> Location { self.0.clone() }
}

/// A number literal decodes to NaN
pub struct NaNLiteral(pub Location);
impl ProjectError for NaNLiteral {
  fn description(&self) -> &str { "float literal decoded to NaN" }
  fn one_position(&self) -> Location { self.0.clone() }
}

/// A sequence of digits in a number literal overflows [usize].
pub struct LiteralOverflow(pub Location);
impl ProjectError for LiteralOverflow {
  fn description(&self) -> &str {
    "number literal described number greater than usize::MAX"
  }
  fn one_position(&self) -> Location { self.0.clone() }
}

/// A digit was expected but something else was found
pub struct ExpectedDigit(pub Location);
impl ProjectError for ExpectedDigit {
  fn description(&self) -> &str { "expected a digit" }
  fn one_position(&self) -> Location { self.0.clone() }
}

/// A unicode escape sequence contains something other than a hex digit
pub struct NotHex(pub Location);
impl ProjectError for NotHex {
  fn description(&self) -> &str { "Expected a hex digit" }
  fn one_position(&self) -> Location { self.0.clone() }
}

/// A unicode escape sequence contains a number that isn't a unicode code point.
pub struct BadCodePoint(pub Location);
impl ProjectError for BadCodePoint {
  fn description(&self) -> &str {
    "\\uXXXX escape sequence does not describe valid code point"
  }
  fn one_position(&self) -> Location { self.0.clone() }
}

/// An unrecognized escape sequence occurred in a string.
pub struct BadEscapeSequence(pub Location);
impl ProjectError for BadEscapeSequence {
  fn description(&self) -> &str { "Unrecognized escape sequence" }
  fn one_position(&self) -> Location { self.0.clone() }
}

/// Expected a parenthesized block at the end of the line
pub struct ExpectedBlock(pub Location);
impl ExpectedBlock {
  /// Remove two parentheses from the ends of the cursor
  pub fn expect(tail: Stream, typ: PType) -> ProjectResult<Stream> {
    let (lp, tail) = tail.trim().pop()?;
    Expected::expect(Lexeme::LP(typ), lp)?;
    let (rp, tail) = tail.pop_back()?;
    Expected::expect(Lexeme::RP(typ), rp)?;
    Ok(tail.trim())
  }
}
impl ProjectError for ExpectedBlock {
  fn description(&self) -> &str { "Expected a parenthesized block" }
  fn one_position(&self) -> Location { self.0.clone() }
}

/// A namespaced name was expected but a glob pattern or a branching multiname
/// was found.
pub struct ExpectedSingleName(pub Location);
impl ProjectError for ExpectedSingleName {
  fn one_position(&self) -> Location { self.0.clone() }
  fn description(&self) -> &str {
    "expected a single name, no wildcards, no branches"
  }
}
