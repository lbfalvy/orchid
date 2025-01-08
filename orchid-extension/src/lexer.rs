use std::ops::{Range, RangeInclusive};

use orchid_base::error::{OrcErr, OrcRes, mk_err};
use orchid_base::intern;
use orchid_base::interner::Tok;
use orchid_base::location::Pos;
use orchid_base::reqnot::{ReqNot, Requester};
use orchid_base::tree::TokHandle;

use crate::api;
use crate::tree::{GenTok, GenTokTree};

pub fn err_cascade() -> OrcErr {
	mk_err(
		intern!(str: "An error cascading from a recursive call"),
		"This error should not surface. If you are seeing it, something is wrong",
		[Pos::None.into()],
	)
}

pub fn err_not_applicable() -> OrcErr {
	mk_err(
		intern!(str: "Pseudo-error to communicate that the current branch in a dispatch doesn't apply"),
		&*err_cascade().message,
		[Pos::None.into()],
	)
}

pub struct LexContext<'a> {
	pub text: &'a Tok<String>,
	pub sys: api::SysId,
	pub id: api::ParsId,
	pub pos: u32,
	pub reqnot: ReqNot<api::ExtMsgSet>,
}
impl<'a> LexContext<'a> {
	pub fn recurse(&self, tail: &'a str) -> OrcRes<(&'a str, GenTokTree<'a>)> {
		let start = self.pos(tail);
		let lx =
			self.reqnot.request(api::SubLex { pos: start, id: self.id }).ok_or_else(err_cascade)?;
		Ok((&self.text[lx.pos as usize..], GenTok::Slot(TokHandle::new(lx.ticket)).at(start..lx.pos)))
	}

	pub fn pos(&self, tail: &'a str) -> u32 { (self.text.len() - tail.len()) as u32 }

	pub fn tok_ran(&self, len: u32, tail: &'a str) -> Range<u32> {
		self.pos(tail) - len..self.pos(tail)
	}
}

pub trait Lexer: Send + Sync + Sized + Default + 'static {
	const CHAR_FILTER: &'static [RangeInclusive<char>];
	fn lex<'a>(tail: &'a str, ctx: &'a LexContext<'a>) -> OrcRes<(&'a str, GenTokTree<'a>)>;
}

pub trait DynLexer: Send + Sync + 'static {
	fn char_filter(&self) -> &'static [RangeInclusive<char>];
	fn lex<'a>(&self, tail: &'a str, ctx: &'a LexContext<'a>) -> OrcRes<(&'a str, GenTokTree<'a>)>;
}

impl<T: Lexer> DynLexer for T {
	fn char_filter(&self) -> &'static [RangeInclusive<char>] { T::CHAR_FILTER }
	fn lex<'a>(&self, tail: &'a str, ctx: &'a LexContext<'a>) -> OrcRes<(&'a str, GenTokTree<'a>)> {
		T::lex(tail, ctx)
	}
}

pub type LexerObj = &'static dyn DynLexer;
