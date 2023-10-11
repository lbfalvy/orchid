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
  #[must_use]
  fn file(&self) -> Arc<VName>;
  #[must_use]
  fn interner(&self) -> &Interner;
  #[must_use]
  fn source(&self) -> Arc<String>;
  fn lexers(&self) -> &[&dyn LexerPlugin];
  fn line_parsers(&self) -> &[&dyn LineParser];
  #[must_use]
  fn pos(&self, tail: &str) -> usize { self.source().len() - tail.len() }
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
  #[must_use]
  fn range_loc(&self, range: Range<usize>) -> Location {
    Location::Range { file: self.file(), range, source: self.source() }
  }
}

pub type LexerPluginOut<'a> = Option<ProjectResult<(Atom, &'a str)>>;
pub type LineParserOut = Option<ProjectResult<Vec<FileEntryKind>>>;
pub trait LexerPlugin:
  for<'a> Fn(&'a str, &dyn Context) -> LexerPluginOut<'a> + Sync + Send
{
}
impl<F> LexerPlugin for F where
  F: for<'a> Fn(&'a str, &dyn Context) -> LexerPluginOut<'a> + Sync + Send
{
}

pub trait LineParser:
  Fn(Stream<'_>, &dyn Context) -> LineParserOut + Sync + Send
{
}
impl<F> LineParser for F where
  F: Fn(Stream<'_>, &dyn Context) -> LineParserOut + Sync + Send
{
}

/// Struct implementing context
///
/// Hiding type parameters in associated types allows for simpler
/// parser definitions
pub struct ParsingContext<'a> {
  pub interner: &'a Interner,
  pub file: Arc<VName>,
  pub source: Arc<String>,
  pub lexers: &'a [&'a dyn LexerPlugin],
  pub line_parsers: &'a [&'a dyn LineParser],
}

impl<'a> ParsingContext<'a> {
  pub fn new(
    interner: &'a Interner,
    file: Arc<VName>,
    source: Arc<String>,
    lexers: &'a [&'a dyn LexerPlugin],
    line_parsers: &'a [&'a dyn LineParser],
  ) -> Self {
    Self { interner, file, source, lexers, line_parsers }
  }
}

impl<'a> Clone for ParsingContext<'a> {
  fn clone(&self) -> Self {
    Self {
      interner: self.interner,
      file: self.file.clone(),
      source: self.source.clone(),
      lexers: self.lexers,
      line_parsers: self.line_parsers,
    }
  }
}

impl Context for ParsingContext<'_> {
  fn interner(&self) -> &Interner { self.interner }
  fn file(&self) -> Arc<VName> { self.file.clone() }
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
