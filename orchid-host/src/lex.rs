use std::sync::Arc;

use async_std::sync::Mutex;
use futures::FutureExt;
use orchid_base::error::{OrcErrv, OrcRes, mk_errv};
use orchid_base::interner::Tok;
use orchid_base::location::Pos;
use orchid_base::parse::{name_char, name_start, op_char, unrep_space};
use orchid_base::tokens::PARENS;
use orchid_base::tree::recur;

use crate::api;
use crate::ctx::Ctx;
use crate::expr::{Expr, ExprParseCtx};
use crate::parsed::{ParsTok, ParsTokTree};
use crate::system::System;

pub struct LexCtx<'a> {
	pub systems: &'a [System],
	pub source: &'a Tok<String>,
	pub tail: &'a str,
	pub sub_trees: &'a mut Vec<Expr>,
	pub ctx: &'a Ctx,
}
impl<'a> LexCtx<'a> {
	pub fn push<'b>(&'b mut self, pos: u32) -> LexCtx<'b>
	where 'a: 'b {
		LexCtx {
			source: self.source,
			tail: &self.source[pos as usize..],
			systems: self.systems,
			sub_trees: &mut *self.sub_trees,
			ctx: self.ctx,
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
	pub async fn ser_subtree(&mut self, subtree: ParsTokTree) -> api::TokenTree {
		let mut exprs = self.ctx.common_exprs.clone();
		let foo = recur(subtree, &|tt, r| {
			if let ParsTok::NewExpr(expr) = tt.tok {
				return ParsTok::Handle(expr).at(tt.range);
			}
			r(tt)
		});
		foo.into_api(&mut exprs, &mut ()).await
	}
	pub async fn des_subtree(&mut self, tree: &api::TokenTree) -> ParsTokTree {
		ParsTokTree::from_api(
			&tree,
			&mut self.ctx.common_exprs.clone(),
			&mut ExprParseCtx { ctx: self.ctx.clone(), exprs: self.ctx.common_exprs.clone() },
			&self.ctx.i,
		)
		.await
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

pub async fn lex_once(ctx: &mut LexCtx<'_>) -> OrcRes<ParsTokTree> {
	let start = ctx.get_pos();
	assert!(
		!ctx.tail.is_empty() && !ctx.tail.starts_with(unrep_space),
		"Lexing empty string or whitespace to token!\n\
    Invocations of lex_tok should check for empty string"
	);
	let tok = if ctx.strip_prefix("\r\n") || ctx.strip_prefix("\r") || ctx.strip_prefix("\n") {
		ParsTok::BR
	} else if let Some(tail) = (ctx.tail.starts_with(name_start).then_some(ctx.tail))
		.and_then(|t| t.trim_start_matches(name_char).strip_prefix("::"))
	{
		let name = &ctx.tail[..ctx.tail.len() - tail.len() - "::".len()];
		let body = lex_once(ctx).boxed_local().await?;
		ParsTok::NS(ctx.ctx.i.i(name).await, Box::new(body))
	} else if ctx.strip_prefix("--[") {
		let Some((cmt, tail)) = ctx.tail.split_once("]--") else {
			return Err(mk_errv(
				ctx.ctx.i.i("Unterminated block comment").await,
				"This block comment has no ending ]--",
				[Pos::Range(start..start + 3).into()],
			));
		};
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
					ctx.ctx.i.i("Unclosed lambda").await,
					"Lambdae started with \\ should separate arguments from body with .",
					[Pos::Range(start..start + 1).into()],
				));
			}
			arg.push(lex_once(ctx).boxed_local().await?);
			ctx.trim_ws();
		}
		ParsTok::LambdaHead(arg)
	} else if let Some((lp, rp, paren)) = PARENS.iter().find(|(lp, ..)| ctx.strip_char(*lp)) {
		let mut body = Vec::new();
		ctx.trim_ws();
		while !ctx.strip_char(*rp) {
			if ctx.tail.is_empty() {
				return Err(mk_errv(
					ctx.ctx.i.i("unclosed paren").await,
					format!("this {lp} has no matching {rp}"),
					[Pos::Range(start..start + 1).into()],
				));
			}
			body.push(lex_once(ctx).boxed_local().await?);
			ctx.trim_ws();
		}
		ParsTok::S(*paren, body)
	} else {
		for sys in ctx.systems {
			let mut errors = Vec::new();
			if ctx.tail.starts_with(|c| sys.can_lex(c)) {
				let (source, pos) = (ctx.source.clone(), ctx.get_pos());
				let ctx_lck = &Mutex::new(&mut *ctx);
				let errors_lck = &Mutex::new(&mut errors);
				let lx = sys
					.lex(source, pos, |pos| async move {
						let mut ctx_g = ctx_lck.lock().await;
						match lex_once(&mut ctx_g.push(pos)).boxed_local().await {
							Ok(t) => Some(api::SubLexed { pos: t.range.end, tree: ctx_g.ser_subtree(t).await }),
							Err(e) => {
								errors_lck.lock().await.push(e);
								None
							},
						}
					})
					.await;
				match lx {
					Err(e) =>
						return Err(
							errors.into_iter().fold(OrcErrv::from_api(&e, &ctx.ctx.i).await, |a, b| a + b),
						),
					Ok(Some(lexed)) => {
						ctx.set_pos(lexed.pos);
						return Ok(ctx.des_subtree(&lexed.expr).await);
					},
					Ok(None) => match errors.into_iter().reduce(|a, b| a + b) {
						Some(errors) => return Err(errors),
						None => continue,
					},
				}
			}
		}
		if ctx.tail.starts_with(name_start) {
			ParsTok::Name(ctx.ctx.i.i(ctx.get_start_matches(name_char)).await)
		} else if ctx.tail.starts_with(op_char) {
			ParsTok::Name(ctx.ctx.i.i(ctx.get_start_matches(op_char)).await)
		} else {
			return Err(mk_errv(
				ctx.ctx.i.i("Unrecognized character").await,
				"The following syntax is meaningless.",
				[Pos::Range(start..start + 1).into()],
			));
		}
	};
	Ok(ParsTokTree { tok, range: start..ctx.get_pos() })
}

pub async fn lex(text: Tok<String>, systems: &[System], ctx: &Ctx) -> OrcRes<Vec<ParsTokTree>> {
	let mut sub_trees = Vec::new();
	let mut ctx = LexCtx { source: &text, sub_trees: &mut sub_trees, tail: &text[..], systems, ctx };
	let mut tokv = Vec::new();
	ctx.trim(unrep_space);
	while !ctx.tail.is_empty() {
		tokv.push(lex_once(&mut ctx).await?);
		ctx.trim(unrep_space);
	}
	Ok(tokv)
}
