use std::num::NonZeroU64;
use std::ops::RangeInclusive;

use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;

use crate::error::OrcResult;
use crate::interner::TStr;
use crate::proto::{ExtHostReq, HostExtReq};
use crate::system::SysId;
use crate::tree::{TokenTree, TreeTicket};

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Coding)]
pub struct ParsId(pub NonZeroU64);

/// - All ranges contain at least one character
/// - All ranges are in increasing characeter order
/// - There are excluded characters between each pair of neighboring ranges
#[derive(Clone, Debug, Coding)]
pub struct CharFilter(pub Vec<RangeInclusive<char>>);

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(HostExtReq)]
#[extendable]
pub enum ParserReq {
  LexExpr(LexExpr),
  ParseLine(ParseLine),
}

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(ParserReq, HostExtReq)]
pub struct LexExpr {
  pub sys: SysId,
  pub id: ParsId,
  pub text: TStr,
  pub pos: u32,
}
impl Request for LexExpr {
  type Response = Option<OrcResult<LexedExpr>>;
}

#[derive(Clone, Debug, Coding)]
pub struct LexedExpr {
  pub pos: u32,
  pub expr: TokenTree,
}

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(ExtHostReq)]
pub struct SubLex {
  pub id: ParsId,
  pub pos: u32,
}
impl Request for SubLex {
  type Response = Option<SubLexed>;
}

#[derive(Clone, Debug, Coding)]
pub struct SubLexed {
  pub pos: u32,
  pub ticket: TreeTicket,
}

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(ParserReq, HostExtReq)]
pub struct ParseLine {
  pub sys: SysId,
  pub line: Vec<TokenTree>,
}
impl Request for ParseLine {
  type Response = OrcResult<Vec<TokenTree>>;
}
