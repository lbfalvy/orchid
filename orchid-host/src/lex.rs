use orchid_api::tree::Paren;
use orchid_base::intern;
use orchid_base::interner::Tok;
use orchid_base::location::Pos;
use orchid_base::number::parse_num;

use crate::extension::System;
use crate::results::{mk_err, num_to_err, OwnedResult};
use crate::tree::{OwnedTok, OwnedTokTree};

pub struct LexCtx<'a> {
  pub systems: &'a [System],
  pub source: Tok<String>,
  pub src: &'a str,
}
impl<'a> LexCtx<'a> {
  pub fn get_pos(&self) -> u32 { self.source.len() as u32 - self.src.len() as u32 }
  pub fn strip_prefix(&mut self, tgt: &str) -> bool {
    if let Some(src) = self.src.strip_prefix(tgt) {
      self.src = src;
      return true;
    }
    false
  }
  pub fn strip_char(&mut self, tgt: char) -> bool {
    if let Some(src) = self.src.strip_prefix(tgt) {
      self.src = src;
      return true;
    }
    false
  }
  pub fn trim(&mut self, filter: impl Fn(char) -> bool) {
    self.src = self.src.trim_start_matches(filter);
  }
  pub fn trim_ws(&mut self, br: bool) {
    self.trim(|c| c.is_whitespace() && br || !"\r\n".contains(c))
  }
  pub fn get_start_matches(&mut self, filter: impl Fn(char) -> bool) -> &'a str {
    let rest = self.src.trim_start_matches(filter);
    let matches = &self.src[..self.src.len() - rest.len()];
    self.src = rest;
    matches
  }
}

const PARENS: &[(char, char, Paren)] =
  &[('(', ')', Paren::Round), ('[', ']', Paren::Square), ('{', '}', Paren::Curly)];

pub fn lex_tok(ctx: &mut LexCtx, br: bool) -> OwnedResult<OwnedTokTree> {
  assert!(
    !ctx.src.is_empty() && !ctx.src.starts_with(char::is_whitespace),
    "Lexing empty string or whitespace to token! Invocations of lex_tok should check for empty string"
  );
  for (open, close, paren) in PARENS {
    let paren_pos = ctx.get_pos();
    if ctx.strip_char(*open) {
      let mut body = Vec::new();
      return loop {
        ctx.trim_ws(true);
        if ctx.strip_char(*close) {
          break Ok(OwnedTokTree {
            tok: OwnedTok::S(paren.clone(), body),
            range: paren_pos..ctx.get_pos(),
          });
        } else if ctx.src.is_empty() {
          return Err(vec![mk_err(
            intern!(str: "unclosed paren"),
            format!("this {open} has no matching {close}"),
            [Pos::Range(paren_pos..paren_pos + 1).into()],
          )]);
        }
        body.push(lex_tok(ctx, true)?);
      };
    }
  }
  if ctx.strip_char('\\') {
    let bs_pos = ctx.get_pos() - 1;
    let mut arg = Vec::new();
    loop {
      ctx.trim_ws(true);
      if ctx.strip_char('.') {
        break;
      } else if ctx.src.is_empty() {
        return Err(vec![mk_err(
          intern!(str: "Unclosed lambda"),
          "Lambdae started with \\ should separate arguments from body with .",
          [Pos::Range(bs_pos..bs_pos + 1).into()],
        )]);
      }
      arg.push(lex_tok(ctx, true)?);
    }
    let mut body = Vec::new();
    return loop {
      ctx.trim_ws(br);
      let pos_before_end = ctx.get_pos();
      if !br && ctx.strip_char('\n')
        || PARENS.iter().any(|(_, e, _)| ctx.strip_char(*e))
        || ctx.src.is_empty()
      {
        break Ok(OwnedTokTree { tok: OwnedTok::Lambda(arg, body), range: bs_pos..pos_before_end });
      }
      body.push(lex_tok(ctx, br)?);
    };
  }
  if ctx.src.starts_with(char::is_numeric) {
    let num_pos = ctx.get_pos();
    let num_str = ctx.get_start_matches(|c| c.is_alphanumeric() || "._".contains(c));
    return Ok(OwnedTokTree {
      range: num_pos..ctx.get_pos(),
      tok: match parse_num(num_str) {
        Err(e) => OwnedTok::Bottom(num_to_err(e, num_pos)),
        Ok(v) => todo!(),
      },
    });
  }
  for sys in ctx.systems {}
  todo!()
}
