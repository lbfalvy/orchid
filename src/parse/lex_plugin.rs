//! Abstractions for dynamic extensions to the lexer to parse custom literals

use super::context::{FlatLocContext, ParseCtx};
use super::lexer::{lex, Entry, LexRes};
use crate::error::ProjectResult;
use crate::location::SourceRange;

/// Data passed to the recursive sub-lexer
pub struct LexPluginRecur<'a, 'b> {
  /// Text to tokenize
  pub tail: &'a str,
  /// Callback that will be called between lexemes on the leftover text.
  /// When it returns true, the lexer exits and leaves the remaining text for
  /// you.
  pub exit: &'b mut dyn for<'c> FnMut(&'c str) -> ProjectResult<bool>,
}

/// Data and actions available to a lexer plugin
pub trait LexPluginReq<'a> {
  /// Text to tokenize
  fn tail(&self) -> &'a str;
  /// [ParseCtx] instance for calculating locations and such
  fn ctx(&self) -> &dyn ParseCtx;
  /// Start a child lexer that calls back between lexemes and exits on your
  /// command. You can combine this with custom atoms to create holes for
  /// expressions in your literals like the template strings of most languages
  /// other than Rust.
  fn recurse(&self, req: LexPluginRecur<'a, '_>) -> ProjectResult<LexRes<'a>>;
  /// Lex an inserted piece of text, especially when translating custom syntax
  /// into multiple lexemes.
  ///
  /// # Panics
  ///
  /// If tokenization fails
  fn insert(&self, data: &str, range: SourceRange) -> Vec<Entry>;
}

/// External plugin that parses a literal into recognized Orchid lexemes, most
/// likely atoms.
pub trait LexerPlugin: Send + Sync {
  /// Run the lexer
  fn lex<'a>(&self, req: &'_ dyn LexPluginReq<'a>) -> Option<ProjectResult<LexRes<'a>>>;
}

pub(super) struct LexPlugReqImpl<'a, 'b, TCtx: ParseCtx> {
  pub tail: &'a str,
  pub ctx: &'b TCtx,
}
impl<'a, 'b, TCtx: ParseCtx> LexPluginReq<'a> for LexPlugReqImpl<'a, 'b, TCtx> {
  fn tail(&self) -> &'a str { self.tail }
  fn ctx(&self) -> &dyn ParseCtx { self.ctx }
  fn recurse(&self, req: LexPluginRecur<'a, '_>) -> ProjectResult<LexRes<'a>> {
    lex(Vec::new(), req.tail, self.ctx, |s| (req.exit)(s))
  }
  fn insert(&self, data: &str, range: SourceRange) -> Vec<Entry> {
    let ctx = FlatLocContext::new(self.ctx as &dyn ParseCtx, &range);
    lex(Vec::new(), data, &ctx, |_| Ok(false))
      .expect("Insert failed to lex")
      .tokens
  }
}
