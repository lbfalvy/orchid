use std::ops::{Range, RangeInclusive};

use orchid_api::error::ReportError;
use orchid_api::parser::{LexId, SubLex};
use orchid_api::proto::ExtMsgSet;
use orchid_api::system::SysId;
use orchid_base::interner::Tok;
use orchid_base::reqnot::{ReqNot, Requester};

use crate::error::{
  err_from_api_or_ref, err_or_ref_to_api, pack_err, unpack_err, ProjectErrorObj, ProjectResult,
};
use crate::tree::{OwnedTok, OwnedTokTree};

pub struct LexContext<'a> {
  pub text: &'a Tok<String>,
  pub sys: SysId,
  pub id: LexId,
  pub pos: u32,
  pub reqnot: ReqNot<ExtMsgSet>,
}
impl<'a> LexContext<'a> {
  pub fn recurse(&self, tail: &'a str) -> ProjectResult<(&'a str, OwnedTokTree)> {
    let start = self.pos(tail);
    self
      .reqnot
      .request(SubLex { pos: start, id: self.id })
      .map_err(|e| pack_err(e.iter().map(|e| err_from_api_or_ref(e, self.reqnot.clone()))))
      .map(|lx| (&self.text[lx.pos as usize..], OwnedTok::Slot(lx.ticket).at(start..lx.pos)))
  }

  pub fn pos(&self, tail: &'a str) -> u32 { (self.text.len() - tail.len()) as u32 }

  pub fn tok_ran(&self, len: u32, tail: &'a str) -> Range<u32> {
    self.pos(tail) - len..self.pos(tail)
  }

  pub fn report(&self, e: ProjectErrorObj) {
    for e in unpack_err(e) {
      self.reqnot.notify(ReportError(self.sys, err_or_ref_to_api(e)))
    }
  }
}

pub trait Lexer: Send + Sync + Sized + Default + 'static {
  const CHAR_FILTER: &'static [RangeInclusive<char>];
  fn lex<'a>(
    tail: &'a str,
    ctx: &'a LexContext<'a>,
  ) -> Option<ProjectResult<(&'a str, OwnedTokTree)>>;
}

pub trait DynLexer: Send + Sync + 'static {
  fn char_filter(&self) -> &'static [RangeInclusive<char>];
  fn lex<'a>(
    &self,
    tail: &'a str,
    ctx: &'a LexContext<'a>,
  ) -> Option<ProjectResult<(&'a str, OwnedTokTree)>>;
}

impl<T: Lexer> DynLexer for T {
  fn char_filter(&self) -> &'static [RangeInclusive<char>] { T::CHAR_FILTER }
  fn lex<'a>(
    &self,
    tail: &'a str,
    ctx: &'a LexContext<'a>,
  ) -> Option<ProjectResult<(&'a str, OwnedTokTree)>> {
    T::lex(tail, ctx)
  }
}

pub type LexerObj = &'static dyn DynLexer;
