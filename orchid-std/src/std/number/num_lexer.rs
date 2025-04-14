use std::ops::RangeInclusive;

use orchid_base::error::OrcRes;
use orchid_base::number::{num_to_err, parse_num};
use orchid_extension::atom::ToAtom;
use orchid_extension::lexer::{LexContext, Lexer};
use orchid_extension::tree::{GenTok, GenTokTree};

use super::num_atom::Num;

#[derive(Default)]
pub struct NumLexer;
impl Lexer for NumLexer {
	const CHAR_FILTER: &'static [RangeInclusive<char>] = &['0'..='9'];
	async fn lex<'a>(all: &'a str, ctx: &'a LexContext<'a>) -> OrcRes<(&'a str, GenTokTree<'a>)> {
		let ends_at = all.find(|c: char| !c.is_ascii_hexdigit() && !"xX._pP".contains(c));
		let (chars, tail) = all.split_at(ends_at.unwrap_or(all.len()));
		let fac = match parse_num(chars) {
			Ok(numeric) => Num(numeric).to_atom_factory(),
			Err(e) => return Err(num_to_err(e, ctx.pos(all), ctx.i).await.into()),
		};
		Ok((tail, GenTok::X(fac).at(ctx.pos(all)..ctx.pos(tail))))
	}
}
