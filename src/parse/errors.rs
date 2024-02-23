//! Errors produced by the parser. Plugins are encouraged to reuse these where
//! applicable.

use intern_all::Tok;
use itertools::Itertools;

use super::context::ParseCtx;
use super::frag::Frag;
use super::lexer::{Entry, Lexeme};
use crate::error::{ProjectError, ProjectErrorObj, ProjectResult};
use crate::location::{CodeOrigin, SourceRange};
use crate::parse::parsed::PType;

/// Parse error information without a location. Location data is added by the
/// parser.
pub trait ParseErrorKind: Sized + Send + Sync + 'static {
  /// A general description of the error condition
  const DESCRIPTION: &'static str;
  /// A specific description of the error with concrete text sections
  fn message(&self) -> String { Self::DESCRIPTION.to_string() }
  /// Convert this error to a type-erased [ProjectError] to be handled together
  /// with other Orchid errors.
  fn pack(self, range: SourceRange) -> ProjectErrorObj { ParseError { kind: self, range }.pack() }
}

struct ParseError<T> {
  pub range: SourceRange,
  pub kind: T,
}
impl<T: ParseErrorKind> ProjectError for ParseError<T> {
  const DESCRIPTION: &'static str = T::DESCRIPTION;
  fn one_position(&self) -> CodeOrigin { self.range.origin() }
  fn message(&self) -> String { self.kind.message() }
}

/// A line does not begin with an identifying keyword. Raised on the first token
pub(super) struct LineNeedsPrefix(pub Lexeme);
impl ParseErrorKind for LineNeedsPrefix {
  const DESCRIPTION: &'static str = "This linetype requires a prefix";
  fn message(&self) -> String { format!("{} cannot appear at the beginning of a line", self.0) }
}

/// The line ends abruptly. Raised on the last token
pub(super) struct UnexpectedEOL(pub Lexeme);
impl ParseErrorKind for UnexpectedEOL {
  const DESCRIPTION: &'static str = "The line ended abruptly";
  fn message(&self) -> String {
    "In Orchid, all line breaks outside parentheses start a new declaration".to_string()
  }
}

/// The line should have ended. Raised on last valid or first excess token
pub(super) struct ExpectedEOL;
impl ParseErrorKind for ExpectedEOL {
  const DESCRIPTION: &'static str = "Expected the end of the line";
}

/// A name was expected.
pub(super) struct ExpectedName(pub Lexeme);
impl ParseErrorKind for ExpectedName {
  const DESCRIPTION: &'static str = "A name was expected";
  fn message(&self) -> String { format!("Expected a name, found {}", self.0) }
}

/// Unwrap a name or operator.
pub(super) fn expect_name(
  Entry { lexeme, range }: &Entry,
  ctx: &(impl ParseCtx + ?Sized),
) -> ProjectResult<Tok<String>> {
  match lexeme {
    Lexeme::Name(n) => Ok(n.clone()),
    lex => Err(ExpectedName(lex.clone()).pack(ctx.range_loc(range))),
  }
}

/// A specific lexeme was expected
pub(super) struct Expected {
  /// The lexemes that would have been acceptable
  pub expected: Vec<Lexeme>,
  /// Whether a name would also have been acceptable (multiname)
  pub or_name: bool,
  /// What was actually found
  pub found: Lexeme,
}
impl ParseErrorKind for Expected {
  const DESCRIPTION: &'static str = "A concrete token was expected";
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
/// Assert that the entry contains exactly the specified lexeme
pub(super) fn expect(l: Lexeme, e: &Entry, ctx: &(impl ParseCtx + ?Sized)) -> ProjectResult<()> {
  if e.lexeme.strict_eq(&l) {
    return Ok(());
  }
  let found = e.lexeme.clone();
  let kind = Expected { expected: vec![l], or_name: false, found };
  Err(kind.pack(ctx.range_loc(&e.range)))
}

/// A token reserved for future use was found in the code
pub(super) struct ReservedToken(pub Lexeme);
impl ParseErrorKind for ReservedToken {
  const DESCRIPTION: &'static str = "Syntax reserved for future use";
  fn message(&self) -> String { format!("{} is a reserved token", self.0) }
}

/// A token was found where it doesn't belong
pub(super) struct BadTokenInRegion {
  /// What was found
  pub lexeme: Lexeme,
  /// Human-readable name of the region where it should not appear
  pub region: &'static str,
}
impl ParseErrorKind for BadTokenInRegion {
  const DESCRIPTION: &'static str = "An unexpected token was found";
  fn message(&self) -> String { format!("{} cannot appear in {}", self.lexeme, self.region) }
}

/// Some construct was searched but not found.
pub(super) struct NotFound(pub &'static str);
impl ParseErrorKind for NotFound {
  const DESCRIPTION: &'static str = "A specific lexeme was expected";
  fn message(&self) -> String { format!("{} was expected", self.0) }
}

/// :: found on its own somewhere other than a general export
pub(super) struct LeadingNS;
impl ParseErrorKind for LeadingNS {
  const DESCRIPTION: &'static str = ":: can only follow a name token";
}

/// Parens don't pair up
pub(super) struct MisalignedParen(pub Lexeme);
impl ParseErrorKind for MisalignedParen {
  const DESCRIPTION: &'static str = "(), [] and {} must always pair up";
  fn message(&self) -> String { format!("This {} has no pair", self.0) }
}

/// Export line contains a complex name
pub(super) struct NamespacedExport;
impl ParseErrorKind for NamespacedExport {
  const DESCRIPTION: &'static str = "Only local names may be exported";
}

/// Export line contains *
pub(super) struct GlobExport;
impl ParseErrorKind for GlobExport {
  const DESCRIPTION: &'static str = "Globstars are not allowed in exports";
}

/// Comment never ends
pub(super) struct NoCommentEnd;
impl ParseErrorKind for NoCommentEnd {
  const DESCRIPTION: &'static str = "a comment was not closed with `]--`";
}

/// A placeholder's priority is a floating point number
pub(super) struct FloatPlacehPrio;
impl ParseErrorKind for FloatPlacehPrio {
  const DESCRIPTION: &'static str =
    "a placeholder priority has a decimal point or a negative exponent";
}

/// A number literal decodes to NaN
pub(super) struct NaNLiteral;
impl ParseErrorKind for NaNLiteral {
  const DESCRIPTION: &'static str = "float literal decoded to NaN";
}

/// A sequence of digits in a number literal overflows [usize].
pub(super) struct LiteralOverflow;
impl ParseErrorKind for LiteralOverflow {
  const DESCRIPTION: &'static str = "number literal described number greater than usize::MAX";
}

/// A digit was expected but something else was found
pub(super) struct ExpectedDigit;
impl ParseErrorKind for ExpectedDigit {
  const DESCRIPTION: &'static str = "expected a digit";
}

/// Expected a parenthesized block at the end of the line
pub(super) struct ExpectedBlock;
impl ParseErrorKind for ExpectedBlock {
  const DESCRIPTION: &'static str = "Expected a parenthesized block";
}
/// Remove two parentheses from the ends of the cursor
pub(super) fn expect_block<'a>(
  tail: Frag<'a>,
  typ: PType,
  ctx: &(impl ParseCtx + ?Sized),
) -> ProjectResult<Frag<'a>> {
  let (lp, tail) = tail.trim().pop(ctx)?;
  expect(Lexeme::LP(typ), lp, ctx)?;
  let (rp, tail) = tail.pop_back(ctx)?;
  expect(Lexeme::RP(typ), rp, ctx)?;
  Ok(tail.trim())
}

/// A namespaced name was expected but a glob pattern or a branching multiname
/// was found.
pub(super) struct ExpectedSingleName;
impl ParseErrorKind for ExpectedSingleName {
  const DESCRIPTION: &'static str = "expected a single name, no wildcards, no branches";
}
