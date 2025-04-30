use std::cell::RefCell;

use futures::FutureExt;
use futures::future::join_all;
use hashbrown::HashMap;
use itertools::Itertools;
use orchid_base::error::{OrcRes, Reporter, mk_err, mk_errv};
use orchid_base::format::fmt;
use orchid_base::interner::{Interner, Tok};
use orchid_base::name::Sym;
use orchid_base::parse::{
	Comment, Import, ParseCtx, Parsed, Snippet, expect_end, line_items, parse_multiname,
	try_pop_no_fluff,
};
use orchid_base::tree::{Paren, TokTree, Token};
use substack::Substack;

use crate::ctx::Ctx;
use crate::expr::{Expr, ExprKind, PathSetBuilder};
use crate::parsed::{Item, ItemKind, ParsTokTree, ParsedMember, ParsedMemberKind, ParsedModule};
use crate::system::System;

type ParsSnippet<'a> = Snippet<'a, Expr, Expr>;

pub struct HostParseCtxImpl<'a> {
	pub ctx: Ctx,
	pub src: Sym,
	pub systems: &'a [System],
	pub reporter: &'a Reporter,
	pub interner: &'a Interner,
	pub consts: RefCell<HashMap<Sym, Vec<ParsTokTree>>>,
}

impl ParseCtx for HostParseCtxImpl<'_> {
	fn reporter(&self) -> &Reporter { self.reporter }
	fn i(&self) -> &Interner { self.interner }
}

impl HostParseCtx for HostParseCtxImpl<'_> {
	fn ctx(&self) -> &Ctx { &self.ctx }
	fn systems(&self) -> impl Iterator<Item = &System> { self.systems.iter() }
	async fn save_const(&self, path: Substack<'_, Tok<String>>, value: Vec<ParsTokTree>) {
		let name = Sym::new(path.unreverse(), self.interner).await.unwrap();
		self.consts.borrow_mut().insert(name, value);
	}
}

pub trait HostParseCtx: ParseCtx {
	fn ctx(&self) -> &Ctx;
	fn systems(&self) -> impl Iterator<Item = &System>;
	async fn save_const(&self, path: Substack<'_, Tok<String>>, value: Vec<ParsTokTree>);
}

pub async fn parse_items(
	ctx: &impl HostParseCtx,
	path: Substack<'_, Tok<String>>,
	items: ParsSnippet<'_>,
) -> OrcRes<Vec<Item>> {
	let lines = line_items(ctx, items).await;
	let line_res =
		join_all(lines.into_iter().map(|p| parse_item(ctx, path.clone(), p.output, p.tail))).await;
	Ok(line_res.into_iter().flat_map(|l| l.ok().into_iter().flatten()).collect())
}

pub async fn parse_item(
	ctx: &impl HostParseCtx,
	path: Substack<'_, Tok<String>>,
	comments: Vec<Comment>,
	item: ParsSnippet<'_>,
) -> OrcRes<Vec<Item>> {
	match item.pop_front() {
		Some((TokTree { tok: Token::Name(n), .. }, postdisc)) => match n {
			n if *n == ctx.i().i("export").await => match try_pop_no_fluff(ctx, postdisc).await? {
				Parsed { output: TokTree { tok: Token::Name(n), .. }, tail } =>
					parse_exportable_item(ctx, path, comments, true, n.clone(), tail).await,
				Parsed { output: TokTree { tok: Token::S(Paren::Round, body), .. }, tail } => {
					expect_end(ctx, tail).await?;
					let mut ok = Vec::new();
					for tt in body {
						let sr = tt.sr.clone();
						match &tt.tok {
							Token::Name(n) =>
								ok.push(Item { comments: comments.clone(), sr, kind: ItemKind::Export(n.clone()) }),
							Token::NS(..) => ctx.reporter().report(mk_err(
								ctx.i().i("Compound export").await,
								"Cannot export compound names (names containing the :: separator)",
								[sr.pos().into()],
							)),
							t => ctx.reporter().report(mk_err(
								ctx.i().i("Invalid export").await,
								format!("Invalid export target {}", fmt(t, ctx.i()).await),
								[sr.pos().into()],
							)),
						}
					}
					expect_end(ctx, tail).await?;
					Ok(ok)
				},
				Parsed { output, tail: _ } => Err(mk_errv(
					ctx.i().i("Malformed export").await,
					"`export` can either prefix other lines or list names inside ( )",
					[output.sr.pos().into()],
				)),
			},
			n if *n == ctx.i().i("import").await => {
				let imports = parse_import(ctx, postdisc).await?;
				Ok(Vec::from_iter(imports.into_iter().map(|t| Item {
					comments: comments.clone(),
					sr: t.sr.clone(),
					kind: ItemKind::Import(t),
				})))
			},
			n => parse_exportable_item(ctx, path, comments, false, n.clone(), postdisc).await,
		},
		Some(_) => Err(mk_errv(
			ctx.i().i("Expected a line type").await,
			"All lines must begin with a keyword",
			[item.sr().pos().into()],
		)),
		None => unreachable!("These lines are filtered and aggregated in earlier stages"),
	}
}

pub async fn parse_import<'a>(
	ctx: &impl HostParseCtx,
	tail: ParsSnippet<'a>,
) -> OrcRes<Vec<Import>> {
	let Parsed { output: imports, tail } = parse_multiname(ctx, tail).await?;
	expect_end(ctx, tail).await?;
	Ok(imports)
}

pub async fn parse_exportable_item<'a>(
	ctx: &impl HostParseCtx,
	path: Substack<'_, Tok<String>>,
	comments: Vec<Comment>,
	exported: bool,
	discr: Tok<String>,
	tail: ParsSnippet<'a>,
) -> OrcRes<Vec<Item>> {
	let path_sym = Sym::new(path.unreverse(), ctx.i()).await.expect("Files should have a namespace");
	let kind = if discr == ctx.i().i("mod").await {
		let (name, body) = parse_module(ctx, path, tail).await?;
		ItemKind::Member(ParsedMember { name, full_name: path_sym, kind: ParsedMemberKind::Mod(body) })
	} else if discr == ctx.i().i("const").await {
		let name = parse_const(ctx, tail, path.clone()).await?;
		ItemKind::Member(ParsedMember { name, full_name: path_sym, kind: ParsedMemberKind::Const })
	} else if let Some(sys) = ctx.systems().find(|s| s.can_parse(discr.clone())) {
		let line = sys.parse(path_sym, tail.to_vec(), exported, comments).await?;
		return parse_items(ctx, path, Snippet::new(tail.prev(), &line)).await;
	} else {
		let ext_lines = ctx.systems().flat_map(System::line_types).join(", ");
		return Err(mk_errv(
			ctx.i().i("Unrecognized line type").await,
			format!("Line types are: const, mod, macro, grammar, {ext_lines}"),
			[tail.prev().sr.pos().into()],
		));
	};
	Ok(vec![Item { comments, sr: tail.sr(), kind }])
}

pub async fn parse_module<'a>(
	ctx: &impl HostParseCtx,
	path: Substack<'_, Tok<String>>,
	tail: ParsSnippet<'a>,
) -> OrcRes<(Tok<String>, ParsedModule)> {
	let (name, tail) = match try_pop_no_fluff(ctx, tail).await? {
		Parsed { output: TokTree { tok: Token::Name(n), .. }, tail } => (n.clone(), tail),
		Parsed { output, .. } => {
			return Err(mk_errv(
				ctx.i().i("Missing module name").await,
				format!("A name was expected, {} was found", fmt(output, ctx.i()).await),
				[output.sr.pos().into()],
			));
		},
	};
	let Parsed { output, tail: surplus } = try_pop_no_fluff(ctx, tail).await?;
	expect_end(ctx, surplus).await?;
	let Some(body) = output.as_s(Paren::Round) else {
		return Err(mk_errv(
			ctx.i().i("Expected module body").await,
			format!("A ( block ) was expected, {} was found", fmt(output, ctx.i()).await),
			[output.sr.pos().into()],
		));
	};
	let path = path.push(name.clone());
	Ok((name, ParsedModule::new(parse_items(ctx, path, body).await?)))
}

pub async fn parse_const<'a>(
	ctx: &impl HostParseCtx,
	tail: ParsSnippet<'a>,
	path: Substack<'_, Tok<String>>,
) -> OrcRes<Tok<String>> {
	let Parsed { output, tail } = try_pop_no_fluff(ctx, tail).await?;
	let Some(name) = output.as_name() else {
		return Err(mk_errv(
			ctx.i().i("Missing module name").await,
			format!("A name was expected, {} was found", fmt(output, ctx.i()).await),
			[output.sr.pos().into()],
		));
	};
	let Parsed { output, tail } = try_pop_no_fluff(ctx, tail).await?;
	if !output.is_kw(ctx.i().i("=").await) {
		return Err(mk_errv(
			ctx.i().i("Missing = separator").await,
			format!("Expected = , found {}", fmt(output, ctx.i()).await),
			[output.sr.pos().into()],
		));
	}
	try_pop_no_fluff(ctx, tail).await?;
	ctx.save_const(path, tail[..].to_vec()).await;
	Ok(name)
}

pub async fn parse_expr(
	ctx: &impl HostParseCtx,
	path: Sym,
	psb: PathSetBuilder<'_, Tok<String>>,
	tail: ParsSnippet<'_>,
) -> OrcRes<Expr> {
	let Some((last_idx, _)) = (tail.iter().enumerate().find(|(_, tt)| tt.as_lambda().is_some()))
		.or_else(|| tail.iter().enumerate().rev().find(|(_, tt)| !tt.is_fluff()))
	else {
		return Err(mk_errv(ctx.i().i("Empty expression").await, "Expression ends abruptly here", [
			tail.sr().pos().into(),
		]));
	};
	let (function, value) = tail.split_at(last_idx as u32);
	let pos = tail.sr().pos();
	if !function.iter().all(TokTree::is_fluff) {
		let (f_psb, x_psb) = psb.split();
		let x_expr = parse_expr(ctx, path.clone(), x_psb, value).boxed_local().await?;
		let f_expr = parse_expr(ctx, path, f_psb, function).boxed_local().await?;
		return Ok(ExprKind::Call(f_expr, x_expr).at(pos));
	}
	let Parsed { output: head, tail } = try_pop_no_fluff(ctx, value).await?;
	match &head.tok {
		Token::BR | Token::Comment(_) => panic!("Fluff skipped"),
		Token::Bottom(b) => Ok(ExprKind::Bottom(b.clone()).at(pos.clone())),
		Token::Handle(expr) => Ok(expr.clone()),
		Token::NS(n, nametail) => {
			let mut nametail = nametail;
			let mut segments = path.iter().chain([n]).cloned().collect_vec();
			while let Token::NS(n, newtail) = &nametail.tok {
				segments.push(n.clone());
				nametail = newtail;
			}
			let Token::Name(n) = &nametail.tok else {
				return Err(mk_errv(
					ctx.i().i("Loose namespace prefix in constant").await,
					"Namespace prefixes in constants must be followed by names",
					[pos.into()],
				));
			};
			segments.push(n.clone());
			Ok(ExprKind::Const(Sym::new(segments, ctx.i()).await.unwrap()).at(pos.clone()))
		},
		Token::LambdaHead(h) => {
			let [TokTree { tok: Token::Name(arg), .. }] = &h[..] else {
				return Err(mk_errv(
					ctx.i().i("Complex lambda binding in constant").await,
					"Lambda args in constants must be identified by a single name",
					[pos.into()],
				));
			};
			let lambda_builder = psb.lambda(arg);
			let body = parse_expr(ctx, path.clone(), lambda_builder.stack(), tail).boxed_local().await?;
			Ok(ExprKind::Lambda(lambda_builder.collect(), body).at(pos.clone()))
		},
		_ => todo!("AAAAAA"), // TODO: todo
	}
}
