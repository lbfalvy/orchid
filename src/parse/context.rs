//! Definition and implementations of the parsing context, which is used

use std::ops::Range;
use std::sync::Arc;

use super::lex_plugin::LexerPlugin;
use super::parse_plugin::ParseLinePlugin;
use crate::location::{SourceCode, SourceRange};
use crate::name::VPath;
use crate::utils::boxed_iter::{box_empty, BoxedIter};
use crate::utils::sequence::Sequence;

/// Trait enclosing all context features
///
/// The main implementation is [ParsingContext]
pub trait ParseCtx {
  /// Get an object describing the file this source code comes from
  #[must_use]
  fn code_info(&self) -> SourceCode;
  /// Get the list of all lexer plugins
  #[must_use]
  fn lexers(&self) -> BoxedIter<'_, &dyn LexerPlugin>;
  /// Get the list of all parser plugins
  #[must_use]
  fn line_parsers(&self) -> BoxedIter<'_, &dyn ParseLinePlugin>;
  /// Find our position in the text given the text we've yet to parse
  #[must_use]
  fn pos(&self, tail: &str) -> usize { self.source().len() - tail.len() }
  /// Generate a location given the length of a token and the unparsed text
  /// after it. See also [Context::range_loc] if the maths gets complex.
  #[must_use]
  fn range(&self, len: usize, tl: &str) -> Range<usize> {
    match self.pos(tl).checked_sub(len) {
      Some(start) => start..self.pos(tl),
      None => {
        panic!("len={len} greater than tail.len()={}; tail={tl:?}", tl.len())
      },
    }
  }
  /// Create a contextful location for error reporting
  #[must_use]
  fn code_range(&self, len: usize, tl: &str) -> SourceRange {
    self.range_loc(&self.range(len, tl))
  }
  /// Create a contentful location from a range directly.
  #[must_use]
  fn range_loc(&self, range: &Range<usize>) -> SourceRange {
    SourceRange { code: self.code_info(), range: range.clone() }
  }
  /// Get a reference to the full source text. This should not be used for
  /// position math.
  #[must_use]
  fn source(&self) -> Arc<String> { self.code_info().source.clone() }
}

impl<'a, C: ParseCtx + 'a + ?Sized> ParseCtx for &'a C {
  fn lexers(&self) -> BoxedIter<'_, &dyn LexerPlugin> { (*self).lexers() }
  fn line_parsers(&self) -> BoxedIter<'_, &dyn ParseLinePlugin> {
    (*self).line_parsers()
  }
  fn pos(&self, tail: &str) -> usize { (*self).pos(tail) }
  fn code_info(&self) -> SourceCode { (*self).code_info() }
  fn source(&self) -> Arc<String> { (*self).source() }
  fn range(&self, l: usize, t: &str) -> Range<usize> { (*self).range(l, t) }
}

/// Struct implementing context
#[derive(Clone)]
pub struct ParseCtxImpl<'a> {
  /// File to be parsed; where it belongs in the tree and its text
  pub code: SourceCode,
  /// Lexer plugins for parsing custom literals
  pub lexers: Sequence<'a, &'a (dyn LexerPlugin + 'a)>,
  /// Parser plugins for parsing custom line structures
  pub line_parsers: Sequence<'a, &'a dyn ParseLinePlugin>,
}
impl<'a> ParseCtx for ParseCtxImpl<'a> {
  // Rust doesn't realize that this lifetime is covariant
  #[allow(clippy::map_identity)]
  fn lexers(&self) -> BoxedIter<'_, &dyn LexerPlugin> {
    Box::new(self.lexers.iter().map(|r| r))
  }
  #[allow(clippy::map_identity)]
  fn line_parsers(&self) -> BoxedIter<'_, &dyn ParseLinePlugin> {
    Box::new(self.line_parsers.iter().map(|r| r))
  }
  fn code_info(&self) -> SourceCode { self.code.clone() }
}

/// Context instance for testing
pub struct MockContext;
impl ParseCtx for MockContext {
  fn pos(&self, tail: &str) -> usize { usize::MAX / 2 - tail.len() }
  // these are expendable
  fn code_info(&self) -> SourceCode {
    SourceCode {
      path: Arc::new(VPath(vec![])),
      source: Arc::new(String::new()),
    }
  }
  fn lexers(&self) -> BoxedIter<'_, &dyn LexerPlugin> { box_empty() }
  fn line_parsers(&self) -> BoxedIter<'_, &dyn ParseLinePlugin> { box_empty() }
}

/// Context that assigns the same location to every subset of the source code.
/// Its main use case is to process source code that was dynamically generated
/// in response to some user code.
pub struct FlatLocContext<'a, C: ParseCtx + ?Sized> {
  sub: &'a C,
  range: &'a SourceRange,
}
impl<'a, C: ParseCtx + ?Sized> FlatLocContext<'a, C> {
  /// Create a new context that will use the same provided range for every
  /// parsed token
  pub fn new(sub: &'a C, range: &'a SourceRange) -> Self { Self { sub, range } }
}
impl<'a, C: ParseCtx + ?Sized> ParseCtx for FlatLocContext<'a, C> {
  fn pos(&self, _: &str) -> usize { 0 }
  fn lexers(&self) -> BoxedIter<'_, &dyn LexerPlugin> { self.sub.lexers() }
  fn line_parsers(&self) -> BoxedIter<'_, &dyn ParseLinePlugin> {
    self.sub.line_parsers()
  }
  fn code_info(&self) -> SourceCode { self.range.code.clone() }
  fn range(&self, _: usize, _: &str) -> Range<usize> {
    self.range.range.clone()
  }
}
