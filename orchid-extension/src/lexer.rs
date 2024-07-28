use std::ops::{Range, RangeInclusive};

use orchid_api::parser::{ParsId, SubLex};
use orchid_api::proto::ExtMsgSet;
use orchid_api::system::SysId;
use orchid_base::interner::Tok;
use orchid_base::location::Pos;
use orchid_base::reqnot::{ReqNot, Requester};

use crate::error::{
  ProjectError, ProjectResult
};
use crate::tree::{GenTok, GenTokTree};

pub struct CascadingError;
impl ProjectError for CascadingError {
  const DESCRIPTION: &'static str = "An error cascading from a recursive sublexer";
  fn message(&self) -> String {
    "This error should not surface. If you are seeing it, something is wrong".to_string()
  }
  fn one_position(&self) -> Pos { Pos::None }
}

pub struct NotApplicableLexerError;
impl ProjectError for NotApplicableLexerError {
  const DESCRIPTION: &'static str = "Pseudo-error to communicate that the lexer doesn't apply";
  fn message(&self) -> String { CascadingError.message() }
  fn one_position(&self) -> Pos { Pos::None }
}

pub struct LexContext<'a> {
  pub text: &'a Tok<String>,
  pub sys: SysId,
  pub id: ParsId,
  pub pos: u32,
  pub reqnot: ReqNot<ExtMsgSet>,
}
impl<'a> LexContext<'a> {
  pub fn recurse(&self, tail: &'a str) -> ProjectResult<(&'a str, GenTokTree)> {
    let start = self.pos(tail);
    let lx = (self.reqnot.request(SubLex { pos: start, id: self.id }))
      .ok_or_else(|| CascadingError.pack())?;
    Ok((&self.text[lx.pos as usize..], GenTok::Slot(lx.ticket).at(start..lx.pos)))
  }

  pub fn pos(&self, tail: &'a str) -> u32 { (self.text.len() - tail.len()) as u32 }

  pub fn tok_ran(&self, len: u32, tail: &'a str) -> Range<u32> {
    self.pos(tail) - len..self.pos(tail)
  }
}

pub trait Lexer: Send + Sync + Sized + Default + 'static {
  const CHAR_FILTER: &'static [RangeInclusive<char>];
  fn lex<'a>(
    tail: &'a str,
    ctx: &'a LexContext<'a>,
  ) -> ProjectResult<(&'a str, GenTokTree)>;
}

pub trait DynLexer: Send + Sync + 'static {
  fn char_filter(&self) -> &'static [RangeInclusive<char>];
  fn lex<'a>(
    &self,
    tail: &'a str,
    ctx: &'a LexContext<'a>,
  ) -> ProjectResult<(&'a str, GenTokTree)>;
}

impl<T: Lexer> DynLexer for T {
  fn char_filter(&self) -> &'static [RangeInclusive<char>] { T::CHAR_FILTER }
  fn lex<'a>(
    &self,
    tail: &'a str,
    ctx: &'a LexContext<'a>,
  ) -> ProjectResult<(&'a str, GenTokTree)> {
    T::lex(tail, ctx)
  }
}

pub type LexerObj = &'static dyn DynLexer;
