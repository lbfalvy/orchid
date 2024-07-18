//! Parse a float or integer. These functions are also used for the macro
//! priority numbers

use std::num::IntErrorKind;
use std::ops::Range;

use ordered_float::NotNan;

use super::context::ParseCtx;
use super::errors::{ExpectedDigit, LiteralOverflow, NaNLiteral, ParseErrorKind};
use super::lex_plugin::LexPluginReq;
#[allow(unused)] // for doc
use super::lex_plugin::LexerPlugin;
use super::lexer::{split_filter, Entry, LexRes, Lexeme};
use crate::error::{ProjectErrorObj, ProjectResult};
use crate::foreign::atom::AtomGenerator;
use crate::foreign::inert::Inert;
use crate::libs::std::number::Numeric;

impl NumError {
  /// Convert into [ProjectErrorObj]
  pub fn into_proj(
    self,
    len: usize,
    tail: &str,
    ctx: &(impl ParseCtx + ?Sized),
  ) -> ProjectErrorObj {
    let start = ctx.source().len() - tail.len() - len + self.range.start;
    let location = ctx.range_loc(&(start..start + self.range.len()));
    match self.kind {
      NumErrorKind::NaN => NaNLiteral.pack(location),
      NumErrorKind::InvalidDigit => ExpectedDigit.pack(location),
      NumErrorKind::Overflow => LiteralOverflow.pack(location),
    }
  }
}



/// [LexerPlugin] for a number literal
#[derive(Clone)]
pub struct NumericLexer;
impl LexerPlugin for NumericLexer {
  fn lex<'b>(&self, req: &'_ dyn LexPluginReq<'b>) -> Option<ProjectResult<LexRes<'b>>> {
    req.tail().chars().next().filter(|c| numstart(*c)).map(|_| {
      let (num_str, tail) = split_filter(req.tail(), numchar);
      let ag = match parse_num(num_str) {
        Ok(Numeric::Float(f)) => AtomGenerator::cloner(Inert(f)),
        Ok(Numeric::Uint(i)) => AtomGenerator::cloner(Inert(i)),
        Err(e) => return Err(e.into_proj(num_str.len(), tail, req.ctx())),
      };
      let range = req.ctx().range(num_str.len(), tail);
      let entry = Entry { lexeme: Lexeme::Atom(ag), range };
      Ok(LexRes { tail, tokens: vec![entry] })
    })
  }
}
