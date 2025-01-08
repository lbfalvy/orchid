use std::num::NonZeroU64;

use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;

use crate::{Comment, HostExtReq, OrcResult, SysId, TokenTree};

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Coding)]
pub struct ParsId(pub NonZeroU64);

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(HostExtReq)]
pub struct ParseLine {
	pub sys: SysId,
	pub comments: Vec<Comment>,
	pub exported: bool,
	pub line: Vec<TokenTree>,
}
impl Request for ParseLine {
	type Response = OrcResult<Vec<TokenTree>>;
}
