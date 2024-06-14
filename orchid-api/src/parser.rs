use std::ops::RangeInclusive;

use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;

use crate::error::ProjResult;
use crate::intern::TStr;
use crate::proto::{ExtHostReq, HostExtReq};
use crate::system::SysId;
use crate::tree::TokenTree;

/// - All ranges contain at least one character
/// - All ranges are in increasing characeter order
/// - There are excluded characters between each pair of neighboring ranges
#[derive(Clone, Debug, Coding)]
pub struct CharFilter(pub Vec<RangeInclusive<char>>);

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(HostExtReq)]
#[extendable]
pub enum ParserReq {
  Lex(Lex),
}

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(ParserReq, HostExtReq)]
pub struct Lex {
  pub sys: SysId,
  pub text: TStr,
  pub pos: u32,
}
impl Request for Lex {
  type Response = Option<ProjResult<Lexed>>;
}

#[derive(Clone, Debug, Coding)]
pub struct Lexed {
  pub pos: u32,
  pub data: TokenTree,
}

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(ExtHostReq)]
pub struct SubLex {
  pub text: TStr,
  pub pos: u32,
}
impl Request for SubLex {
  type Response = ProjResult<Lexed>;
}

pub struct ParseLine {}
