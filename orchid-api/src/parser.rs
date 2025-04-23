use core::ops::Range;
use std::num::NonZeroU64;

use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;

use crate::{HostExtReq, OrcResult, SysId, TStr, TStrv, TokenTree};

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Coding)]
pub struct ParsId(pub NonZeroU64);

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(HostExtReq)]
pub struct ParseLine {
	pub sys: SysId,
	pub module: TStrv,
	pub comments: Vec<Comment>,
	pub exported: bool,
	pub line: Vec<TokenTree>,
}
impl Request for ParseLine {
	type Response = OrcResult<Vec<TokenTree>>;
}

#[derive(Clone, Debug, Coding)]
pub struct Comment {
	pub text: TStr,
	pub range: Range<u32>,
}
