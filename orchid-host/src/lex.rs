use std::num::NonZeroU64;
use std::sync::Arc;

use hashbrown::HashMap;
use orchid_base::error::{OrcErrv, OrcRes, mk_errv};
use orchid_base::interner::{Tok, intern};
use orchid_base::location::Pos;
use orchid_base::number::{num_to_err, parse_num};
use orchid_base::parse::{name_char, name_start, op_char, unrep_space};
use orchid_base::tokens::PARENS;
use orchid_base::tree::Ph;
use orchid_base::{intern, match_mapping};

use crate::api;
use crate::extension::{AtomHand, System};
use crate::tree::{ParsTok, ParsTokTree};

pub struct LexCtx<'a> {
	pub systems: &'a [System],
	pub source: &'a Tok<String>,
	pub tail: &'a str,
	pub sub_trees: &'a mut HashMap<api::TreeTicket, ParsTokTree>,
}
impl<'a> LexCtx<'a> {
	pub fn push<'b>(&'b mut self, pos: u32) -> LexCtx<'b>
	where 'a: 'b {
		LexCtx {
			source: self.source,
			tail: &self.source[pos as usize..],
			systems: self.systems,
			sub_trees: &mut *self.sub_trees,
		}
	}
	pub fn get_pos(&self) -> u32 { self.end_pos() - self.tail.len() as u32 }
	pub fn end_pos(&self) -> u32 { self.source.len() as u32 }
	pub fn set_pos(&mut self, pos: u32) { self.tail = &self.source[pos as usize..] }
	pub fn push_pos(&mut self, delta: u32) { self.set_pos(self.get_pos() + delta) }
	pub fn set_tail(&mut self, tail: &'a str) { self.tail = tail }
	pub fn strip_prefix(&mut self, tgt: &str) -> bool {
		if let Some(src) = self.tail.strip_prefix(tgt) {
			self.tail = src;
			return true;
		}
		false
	}
	pub fn add_subtree(&mut self, subtree: ParsTokTree) -> api::TreeTicket {
		let next_idx = api::TreeTicket(NonZeroU64::new(self.sub_trees.len() as u64 + 1).unwrap());
		self.sub_trees.insert(next_idx, subtree);
		next_idx
	}
	pub fn rm_subtree(&mut self, ticket: api::TreeTicket) -> ParsTokTree {
		self.sub_trees.remove(&ticket).unwrap()
	}
	pub fn strip_char(&mut self, tgt: char) -> bool {
		if let Some(src) = self.tail.strip_prefix(tgt) {
			self.tail = src;
			return true;
		}
		false
	}
	pub fn trim(&mut self, filter: impl Fn(char) -> bool) {
		self.tail = self.tail.trim_start_matches(filter);
	}
	pub fn trim_ws(&mut self) { self.trim(|c| c.is_whitespace() && !"\r\n".contains(c)) }
	pub fn get_start_matches(&mut self, filter: impl Fn(char) -> bool) -> &'a str {
		let rest = self.tail.trim_start_matches(filter);
		let matches = &self.tail[..self.tail.len() - rest.len()];
		self.tail = rest;
		matches
	}
}

pub fn lex_once(ctx: &mut LexCtx) -> OrcRes<ParsTokTree> {
	let start = ctx.get_pos();
	assert!(
		!ctx.tail.is_empty() && !ctx.tail.starts_with(unrep_space),
		"Lexing empty string or whitespace to token!\n\
    Invocations of lex_tok should check for empty string"
	);
	let tok = if ctx.strip_prefix("\r\n") || ctx.strip_prefix("\r") || ctx.strip_prefix("\n") {
		ParsTok::BR
	} else if ctx.strip_prefix("::") {
		ParsTok::NS
	} else if ctx.strip_prefix("--[") {
		let (cmt, tail) = ctx.tail.split_once("]--").ok_or_else(|| {
			mk_errv(intern!(str: "Unterminated block comment"), "This block comment has no ending ]--", [
				Pos::Range(start..start + 3).into(),
			])
		})?;
		ctx.set_tail(tail);
		ParsTok::Comment(Arc::new(cmt.to_string()))
	} else if let Some(tail) = ctx.tail.strip_prefix("--").filter(|t| !t.starts_with(op_char)) {
		let end = tail.find(['\n', '\r']).map_or(tail.len(), |n| n - 1);
		ctx.push_pos(end as u32);
		ParsTok::Comment(Arc::new(tail[2..end].to_string()))
	} else if ctx.strip_char('\\') {
		let mut arg = Vec::new();
		ctx.trim_ws();
		while !ctx.strip_char('.') {
			if ctx.tail.is_empty() {
				return Err(mk_errv(
					intern!(str: "Unclosed lambda"),
					"Lambdae started with \\ should separate arguments from body with .",
					[Pos::Range(start..start + 1).into()],
				));
			}
			arg.push(lex_once(ctx)?);
			ctx.trim_ws();
		}
		ParsTok::LambdaHead(arg)
	} else if let Some((lp, rp, paren)) = PARENS.iter().find(|(lp, ..)| ctx.strip_char(*lp)) {
		let mut body = Vec::new();
		ctx.trim_ws();
		while !ctx.strip_char(*rp) {
			if ctx.tail.is_empty() {
				return Err(mk_errv(
					intern!(str: "unclosed paren"),
					format!("this {lp} has no matching {rp}"),
					[Pos::Range(start..start + 1).into()],
				));
			}
			body.push(lex_once(ctx)?);
			ctx.trim_ws();
		}
		ParsTok::S(*paren, body)
	} else if ctx.strip_prefix("macro")
		&& !ctx.tail.chars().next().is_some_and(|x| x.is_ascii_alphabetic())
	{
		ctx.strip_prefix("macro");
		if ctx.strip_char('(') {
			let pos = ctx.get_pos();
			let numstr = ctx.get_start_matches(|x| x != ')').trim();
			let num = parse_num(numstr).map_err(|e| num_to_err(e, pos))?;
			ParsTok::Macro(Some(num.to_f64()))
		} else {
			ParsTok::Macro(None)
		}
	} else {
		for sys in ctx.systems {
			let mut errors = Vec::new();
			if ctx.tail.starts_with(|c| sys.can_lex(c)) {
				let lx =
					sys.lex(ctx.source.clone(), ctx.get_pos(), |pos| match lex_once(&mut ctx.push(pos)) {
						Ok(t) => Some(api::SubLexed { pos, ticket: ctx.add_subtree(t) }),
						Err(e) => {
							errors.push(e);
							None
						},
					});
				match lx {
					Err(e) => return Err(errors.into_iter().fold(OrcErrv::from_api(&e), |a, b| a + b)),
					Ok(Some(lexed)) => return Ok(tt_to_owned(&lexed.expr, &mut ctx.push(lexed.pos))),
					Ok(None) => match errors.into_iter().reduce(|a, b| a + b) {
						Some(errors) => return Err(errors),
						None => continue,
					},
				}
			}
		}
		if ctx.tail.starts_with(name_start) {
			ParsTok::Name(intern(ctx.get_start_matches(name_char)))
		} else if ctx.tail.starts_with(op_char) {
			ParsTok::Name(intern(ctx.get_start_matches(op_char)))
		} else {
			return Err(mk_errv(
				intern!(str: "Unrecognized character"),
				"The following syntax is meaningless.",
				[Pos::Range(start..start + 1).into()],
			));
		}
	};
	Ok(ParsTokTree { tok, range: start..ctx.get_pos() })
}

fn tt_to_owned(api: &api::TokenTree, ctx: &mut LexCtx<'_>) -> ParsTokTree {
	let tok = match_mapping!(&api.token, api::Token => ParsTok {
		Atom(atom => AtomHand::from_api(atom.clone())),
		Bottom(err => OrcErrv::from_api(err)),
		LambdaHead(arg => ttv_to_owned(arg, ctx)),
		Name(name => Tok::from_api(*name)),
		S(p.clone(), b.iter().map(|t| tt_to_owned(t, ctx)).collect()),
		BR, NS,
		Comment(c.clone()),
		Ph(ph => Ph::from_api(ph)),
		Macro(*prio),
	} {
		api::Token::Slot(id) => return ctx.rm_subtree(*id),
	});
	ParsTokTree { range: api.range.clone(), tok }
}

fn ttv_to_owned<'a>(
	api: impl IntoIterator<Item = &'a api::TokenTree>,
	ctx: &mut LexCtx<'_>,
) -> Vec<ParsTokTree> {
	api.into_iter().map(|t| tt_to_owned(t, ctx)).collect()
}

pub fn lex(text: Tok<String>, systems: &[System]) -> OrcRes<Vec<ParsTokTree>> {
	let mut sub_trees = HashMap::new();
	let mut ctx = LexCtx { source: &text, sub_trees: &mut sub_trees, tail: &text[..], systems };
	let mut tokv = Vec::new();
	ctx.trim(unrep_space);
	while !ctx.tail.is_empty() {
		tokv.push(lex_once(&mut ctx)?);
		ctx.trim(unrep_space);
	}
	Ok(tokv)
}
