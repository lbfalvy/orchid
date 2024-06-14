use std::ops::RangeInclusive;

use orchid_api::error::ProjResult;
use orchid_api::tree::TokenTree;

pub trait Lexer: Send + Sync + Sized + Default + 'static {
  const CHAR_FILTER: &'static [RangeInclusive<char>];
  fn lex<'a>(
    tail: &'a str,
    recur: impl FnMut(&'a str) -> ProjResult<(&'a str, TokenTree)>,
  ) -> Option<ProjResult<(&'a str, TokenTree)>>;
}

pub trait DynLexer: Send + Sync + 'static {
  fn char_filter(&self) -> &'static [RangeInclusive<char>];
  fn lex<'a>(
    &self,
    tail: &'a str,
    recur: &mut dyn FnMut(&'a str) -> ProjResult<(&'a str, TokenTree)>,
  ) -> Option<ProjResult<(&'a str, TokenTree)>>;
}

impl<T: Lexer> DynLexer for T {
  fn char_filter(&self) -> &'static [RangeInclusive<char>] { T::CHAR_FILTER }
  fn lex<'a>(
    &self,
    tail: &'a str,
    recur: &mut dyn FnMut(&'a str) -> ProjResult<(&'a str, TokenTree)>,
  ) -> Option<ProjResult<(&'a str, TokenTree)>> {
    T::lex(tail, recur)
  }
}

pub type LexerObj = &'static dyn DynLexer;
