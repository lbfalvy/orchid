//! Definition and implementations of the parsing context, which is used

use std::ops::Range;
use std::sync::Arc;

use super::lex_plugin::LexerPlugin;
use super::parse_plugin::ParseLinePlugin;
use crate::error::Reporter;
use crate::location::{SourceCode, SourceRange};
use crate::utils::boxed_iter::{box_empty, BoxedIter};
use crate::utils::sequence::Sequence;

/// Trait enclosing all context features
///
/// The main implementation is [ParseCtxImpl]
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
  /// Error reporter
  #[must_use]
  fn reporter(&self) -> &Reporter;
  /// Find our position in the text given the text we've yet to parse
  #[must_use]
  fn pos(&self, tail: &str) -> usize {
    let tail_len = tail.len();
    let source_len = self.source().len();
    (self.source().len().checked_sub(tail.len())).unwrap_or_else(|| {
      panic!("tail.len()={tail_len} greater than self.source().len()={source_len}; tail={tail:?}")
    })
  }
  /// Generate a location given the length of a token and the unparsed text
  /// after it. See also [ParseCtx::range_loc] if the maths gets complex.
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
  fn source_range(&self, len: usize, tl: &str) -> SourceRange {
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
  fn source(&self) -> Arc<String> { self.code_info().text.clone() }
}

impl<'a, C: ParseCtx + 'a + ?Sized> ParseCtx for &'a C {
  fn reporter(&self) -> &Reporter { (*self).reporter() }
  fn lexers(&self) -> BoxedIter<'_, &dyn LexerPlugin> { (*self).lexers() }
  fn line_parsers(&self) -> BoxedIter<'_, &dyn ParseLinePlugin> { (*self).line_parsers() }
  fn pos(&self, tail: &str) -> usize { (*self).pos(tail) }
  fn code_info(&self) -> SourceCode { (*self).code_info() }
  fn source(&self) -> Arc<String> { (*self).source() }
  fn range(&self, l: usize, t: &str) -> Range<usize> { (*self).range(l, t) }
}

/// Struct implementing context
#[derive(Clone)]
pub struct ParseCtxImpl<'a, 'b> {
  /// File to be parsed; where it belongs in the tree and its text
  pub code: SourceCode,
  /// Error aggregator
  pub reporter: &'b Reporter,
  /// Lexer plugins for parsing custom literals
  pub lexers: Sequence<'a, &'a (dyn LexerPlugin + 'a)>,
  /// Parser plugins for parsing custom line structures
  pub line_parsers: Sequence<'a, &'a dyn ParseLinePlugin>,
}
impl<'a, 'b> ParseCtx for ParseCtxImpl<'a, 'b> {
  fn reporter(&self) -> &Reporter { self.reporter }
  // Rust doesn't realize that this lifetime is covariant
  #[allow(clippy::map_identity)]
  fn lexers(&self) -> BoxedIter<'_, &dyn LexerPlugin> { Box::new(self.lexers.iter().map(|r| r)) }
  #[allow(clippy::map_identity)]
  fn line_parsers(&self) -> BoxedIter<'_, &dyn ParseLinePlugin> {
    Box::new(self.line_parsers.iter().map(|r| r))
  }
  fn code_info(&self) -> SourceCode { self.code.clone() }
}

/// Context instance for testing. Implicitly provides a reporter and panics if
/// any errors are reported
pub struct MockContext(pub Reporter);
impl MockContext {
  /// Create a new mock
  pub fn new() -> Self { Self(Reporter::new()) }
}
impl Default for MockContext {
  fn default() -> Self { Self::new() }
}
impl ParseCtx for MockContext {
  fn reporter(&self) -> &Reporter { &self.0 }
  fn pos(&self, tail: &str) -> usize { usize::MAX / 2 - tail.len() }
  // these are expendable
  fn code_info(&self) -> SourceCode { SourceRange::mock().code() }
  fn lexers(&self) -> BoxedIter<'_, &dyn LexerPlugin> { box_empty() }
  fn line_parsers(&self) -> BoxedIter<'_, &dyn ParseLinePlugin> { box_empty() }
}
impl Drop for MockContext {
  fn drop(&mut self) { self.0.assert() }
}

/// Context that assigns the same location to every subset of the source code.
/// Its main use case is to process source code that was dynamically generated
/// in response to some user code. See also [ReporterContext]
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
  fn reporter(&self) -> &Reporter { self.sub.reporter() }
  fn pos(&self, _: &str) -> usize { 0 }
  fn lexers(&self) -> BoxedIter<'_, &dyn LexerPlugin> { self.sub.lexers() }
  fn line_parsers(&self) -> BoxedIter<'_, &dyn ParseLinePlugin> { self.sub.line_parsers() }
  fn code_info(&self) -> SourceCode { self.range.code.clone() }
  fn range(&self, _: usize, _: &str) -> Range<usize> { self.range.range.clone() }
}

/// Context that forwards everything to a wrapped context except for error
/// reporting. See also [FlatLocContext]
pub struct ReporterContext<'a, C: ParseCtx + ?Sized> {
  sub: &'a C,
  reporter: &'a Reporter,
}
impl<'a, C: ParseCtx + ?Sized> ReporterContext<'a, C> {
  /// Create a new context that will collect errors separately and forward
  /// everything else to an enclosed context
  pub fn new(sub: &'a C, reporter: &'a Reporter) -> Self { Self { sub, reporter } }
}
impl<'a, C: ParseCtx + ?Sized> ParseCtx for ReporterContext<'a, C> {
  fn reporter(&self) -> &Reporter { self.reporter }
  fn pos(&self, tail: &str) -> usize { self.sub.pos(tail) }
  fn lexers(&self) -> BoxedIter<'_, &dyn LexerPlugin> { self.sub.lexers() }
  fn line_parsers(&self) -> BoxedIter<'_, &dyn ParseLinePlugin> { self.sub.line_parsers() }
  fn code_info(&self) -> SourceCode { self.sub.code_info() }
  fn range(&self, len: usize, tl: &str) -> Range<usize> { self.sub.range(len, tl) }
  fn range_loc(&self, range: &Range<usize>) -> SourceRange { self.sub.range_loc(range) }
  fn source(&self) -> Arc<String> { self.sub.source() }
  fn source_range(&self, len: usize, tl: &str) -> SourceRange { self.sub.source_range(len, tl) }
}
