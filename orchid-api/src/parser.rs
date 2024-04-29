use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;

use crate::intern::TStr;
use crate::proto::{ExtHostReq, HostExtReq};
use crate::system::SysId;
use crate::tree::TokenTree;

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(HostExtReq)]
#[extendable]
pub enum ParserReq {
  MkLexer(MkLexer),
  Lex(Lex),
}

pub type LexerId = u16;

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(ParserReq, HostExtReq)]
pub struct MkLexer(pub SysId, pub TStr);
impl Request for MkLexer {
  type Response = LexerId;
}

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(ParserReq, HostExtReq)]
pub struct Lex {
  pub parser: LexerId,
  pub next: char,
  pub pos: u32,
}
impl Request for Lex {
  type Response = Option<LexResult>;
}

#[derive(Clone, Debug, Coding)]
pub struct LexResult {
  pub consumed: u32,
  pub data: TokenTree,
}

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(ExtHostReq)]
pub struct SubLex {
  pub lexer: LexerId,
  pub pos: u32,
}
impl Request for SubLex {
  type Response = SubLex;
}

pub struct ParseLine {

}
