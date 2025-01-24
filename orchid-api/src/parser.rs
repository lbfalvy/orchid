use std::num::NonZeroU64;

use orchid_api_derive::{Coding, Decode, Encode, Hierarchy};
use orchid_api_traits::Request;

use crate::{Comment, HostExtReq, OrcResult, SysId, TokenTree};

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct ParsId(pub NonZeroU64);

// impl orchid_api_traits::Decode for ParsId {
// 	async fn decode<R: async_std::io::Read + ?Sized>(mut read:
// std::pin::Pin<&mut R>) -> Self {
// 		Self(orchid_api_traits::Decode::decode(read.as_mut()).await)
// 	}
// }

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
