use std::ops::Range;
use std::sync::Arc;

use super::stream::Stream;
use crate::error::ProjectResult;
use crate::foreign::Atom;
use crate::interner::Interner;
use crate::sourcefile::FileEntryKind;
use crate::{Location, VName};

/// Trait enclosing all context features
///
/// Hiding type parameters in associated types allows for simpler
/// parser definitions
pub trait Context {
  /// Get the path to the current file
  #[must_use]
  fn file(&self) -> Arc<VName>;
  /// Get a reference to the interner
  #[must_use]
  fn interner(&self) -> &Interner;
  /// Get a reference to the full source text for position math and to build
  /// [Location]s.
  #[must_use]
  fn source(&self) -> Arc<String>;
  /// Get the list of all lexer plugins
  #[must_use]
  fn lexers(&self) -> &[&dyn LexerPlugin];
  /// Get the list of all parser plugins
  #[must_use]
  fn line_parsers(&self) -> &[&dyn LineParser];
  /// Find our position in the text given the text we've yet to parse
  #[must_use]
  fn pos(&self, tail: &str) -> usize { self.source().len() - tail.len() }
  /// Generate a location given the length of a token and the unparsed text
  /// after it. See also [Context::range_loc] if the maths gets complex.
  #[must_use]
  fn location(&self, len: usize, tail: &str) -> Location {
    match self.pos(tail).checked_sub(len) {
      Some(start) => self.range_loc(start..self.pos(tail)),
      None => {
        let tl = tail.len();
        panic!("len={len} greater than tail.len()={tl}; tail={tail:?}")
      },
    }
  }
  /// Generate a location given a range in the source file. The location can be
  /// computed with [Context::pos]. See also [Context::location].
  #[must_use]
  fn range_loc(&self, range: Range<usize>) -> Location {
    Location::Range { file: self.file(), range, source: self.source() }
  }
}

impl<C: Context + ?Sized> Context for &C {
  fn file(&self) -> Arc<VName> { (*self).file() }
  fn interner(&self) -> &Interner { (*self).interner() }
  fn lexers(&self) -> &[&dyn LexerPlugin] { (*self).lexers() }
  fn line_parsers(&self) -> &[&dyn LineParser] { (*self).line_parsers() }
  fn location(&self, len: usize, tail: &str) -> Location {
    (*self).location(len, tail)
  }
  fn pos(&self, tail: &str) -> usize { (*self).pos(tail) }
  fn range_loc(&self, range: Range<usize>) -> Location {
    (*self).range_loc(range)
  }
  fn source(&self) -> Arc<String> { (*self).source() }
}

/// Return value of a lexer plugin; the parsed data and the remaining string
pub type LexerPluginOut<'a> = Option<ProjectResult<(Atom, &'a str)>>;
/// Return value of a line parser; the meaningful lines derived from this parser
pub type LineParserOut = Option<ProjectResult<Vec<FileEntryKind>>>;

/// A plugin callback that reads a custom lexeme.
pub trait LexerPlugin:
  for<'a> Fn(&'a str, &dyn Context) -> LexerPluginOut<'a> + Sync + Send
{
}
impl<F> LexerPlugin for F where
  F: for<'a> Fn(&'a str, &dyn Context) -> LexerPluginOut<'a> + Sync + Send
{
}

/// A plugin callback that parses a custom file entry
pub trait LineParser:
  for<'a> Fn(Stream<'_>, &'a (dyn Context + 'a)) -> LineParserOut
  + Sync
  + Send
{
}
impl<F> LineParser for F where
  F: for<'a> Fn(Stream<'_>, &'a (dyn Context + 'a)) -> LineParserOut
    + Sync
    + Send
{
}

/// Struct implementing context
///
/// Hiding type parameters in associated types allows for simpler
/// parser definitions
pub struct ParsingContext<'a> {
  interner: &'a Interner,
  file_path: Arc<VName>,
  source: Arc<String>,
  lexers: &'a [&'a dyn LexerPlugin],
  line_parsers: &'a [&'a dyn LineParser],
}

impl<'a> ParsingContext<'a> {
  /// Create a new parsing context
  pub fn new(
    interner: &'a Interner,
    file_path: Arc<VName>,
    source: Arc<String>,
    lexers: &'a [&'a dyn LexerPlugin],
    line_parsers: &'a [&'a dyn LineParser],
  ) -> Self {
    Self { interner, file_path, source, lexers, line_parsers }
  }
}

impl<'a> Clone for ParsingContext<'a> {
  fn clone(&self) -> Self {
    Self {
      interner: self.interner,
      file_path: self.file_path.clone(),
      source: self.source.clone(),
      lexers: self.lexers,
      line_parsers: self.line_parsers,
    }
  }
}

impl Context for ParsingContext<'_> {
  fn interner(&self) -> &Interner { self.interner }
  fn file(&self) -> Arc<VName> { self.file_path.clone() }
  fn source(&self) -> Arc<String> { self.source.clone() }
  fn lexers(&self) -> &[&dyn LexerPlugin] { self.lexers }
  fn line_parsers(&self) -> &[&dyn LineParser] { self.line_parsers }
}

pub struct MockContext<'a>(pub &'a Interner);
impl<'a> Context for MockContext<'a> {
  // these are doing something
  fn interner(&self) -> &Interner { self.0 }
  fn pos(&self, tail: &str) -> usize { usize::MAX / 2 - tail.len() }
  // these are expendable
  fn file(&self) -> Arc<VName> { Arc::new(Vec::new()) }
  fn lexers(&self) -> &[&dyn LexerPlugin] { &[] }
  fn line_parsers(&self) -> &[&dyn LineParser] { &[] }
  fn location(&self, _: usize, _: &str) -> Location { Location::Unknown }
  fn range_loc(&self, _: Range<usize>) -> Location { Location::Unknown }
  fn source(&self) -> Arc<String> { Arc::new(String::new()) }
}

pub struct FlatLocContext<'a, C: Context + ?Sized> {
  sub: &'a C,
  location: &'a Location,
}
impl<'a, C: Context + ?Sized> FlatLocContext<'a, C> {
  pub fn new(sub: &'a C, location: &'a Location) -> Self {
    Self { sub, location }
  }
}
impl<'a, C: Context + ?Sized> Context for FlatLocContext<'a, C> {
  fn interner(&self) -> &Interner { self.sub.interner() }
  fn pos(&self, _: &str) -> usize { 0 }
  fn file(&self) -> Arc<VName> { self.sub.file() }
  fn lexers(&self) -> &[&dyn LexerPlugin] { self.sub.lexers() }
  fn line_parsers(&self) -> &[&dyn LineParser] { self.sub.line_parsers() }
  fn source(&self) -> Arc<String> { self.sub.source() }
  fn location(&self, _: usize, _: &str) -> Location { self.location.clone() }
  fn range_loc(&self, _: Range<usize>) -> Location { self.location.clone() }
}
