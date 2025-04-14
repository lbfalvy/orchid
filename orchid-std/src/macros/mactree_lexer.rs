use std::ops::RangeInclusive;
use std::rc::Rc;

use futures::FutureExt;
use orchid_base::error::{OrcRes, mk_errv};
use orchid_base::location::Pos;
use orchid_base::parse::name_start;
use orchid_base::tokens::PARENS;
use orchid_extension::atom::AtomicFeatures;
use orchid_extension::gen_expr::atom;
use orchid_extension::lexer::{LexContext, Lexer, err_not_applicable};
use orchid_extension::tree::{GenTok, GenTokTree};

use crate::macros::mactree::{MacTok, MacTree};

#[derive(Default)]
pub struct MacTreeLexer;
impl Lexer for MacTreeLexer {
	const CHAR_FILTER: &'static [RangeInclusive<char>] = &['\''..='\''];
	async fn lex<'a>(tail: &'a str, ctx: &'a LexContext<'a>) -> OrcRes<(&'a str, GenTokTree<'a>)> {
		let Some(tail2) = tail.strip_prefix('\'') else {
			return Err(err_not_applicable(ctx.i).await.into());
		};
		let tail3 = tail2.trim_start();
		return match mac_tree(tail3, ctx).await {
			Ok((tail4, mactree)) =>
				Ok((tail4, GenTok::X(mactree.factory()).at(ctx.pos(tail)..ctx.pos(tail4)))),
			Err(e) => Ok((tail2, GenTok::Bottom(e).at(ctx.tok_ran(1, tail2)))),
		};
		async fn mac_tree<'a>(tail: &'a str, ctx: &'a LexContext<'a>) -> OrcRes<(&'a str, MacTree)> {
			for (lp, rp, paren) in PARENS {
				let Some(mut body_tail) = tail.strip_prefix(*lp) else { continue };
				let mut items = Vec::new();
				return loop {
					let tail2 = body_tail.trim();
					if let Some(tail3) = tail2.strip_prefix(*rp) {
						break Ok((tail3, MacTree {
							pos: Pos::Range(ctx.pos(tail)..ctx.pos(tail3)),
							tok: Rc::new(MacTok::S(*paren, items)),
						}));
					} else if tail2.is_empty() {
						return Err(mk_errv(
							ctx.i.i("Unclosed block").await,
							format!("Expected closing {rp}"),
							[Pos::Range(ctx.tok_ran(1, tail)).into()],
						));
					}
					let (new_tail, new_item) = mac_tree(tail2, ctx).boxed_local().await?;
					body_tail = new_tail;
					items.push(new_item);
				};
			}
			const INTERPOL: &[&str] = &["$", "..$"];
			for pref in INTERPOL {
				let Some(code) = tail.strip_prefix(pref) else { continue };
			}
			return Err(mk_errv(
				ctx.i.i("Expected token after '").await,
				format!("Expected a token after ', found {tail:?}"),
				[Pos::Range(ctx.tok_ran(1, tail)).into()],
			));
		}
	}
}
