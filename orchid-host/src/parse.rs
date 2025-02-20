use std::rc::Rc;

use futures::FutureExt;
use futures::future::join_all;
use itertools::Itertools;
use never::Never;
use orchid_base::error::{OrcErrv, OrcRes, Reporter, ReporterImpl, mk_err, mk_errv};
use orchid_base::interner::Tok;
use orchid_base::location::Pos;
use orchid_base::macros::{MTok, MTree};
use orchid_base::name::Sym;
use orchid_base::parse::{
	Comment, Import, Parsed, Snippet, expect_end, line_items, parse_multiname, try_pop_no_fluff,
};
use orchid_base::tree::{Paren, TokTree, Token};
use substack::Substack;

use crate::atom::AtomHand;
use crate::macros::MacTree;
use crate::system::System;
use crate::tree::{Code, CodeLocator, Item, ItemKind, Member, MemberKind, Module, Rule, RuleKind};

type ParsSnippet<'a> = Snippet<'a, 'static, AtomHand, Never>;

pub struct ParseCtxImpl<'a> {
	pub systems: &'a [System],
	pub reporter: &'a ReporterImpl,
}

impl ParseCtx for ParseCtxImpl<'_> {
	fn reporter(&self) -> &(impl Reporter + ?Sized) { self.reporter }
	fn systems(&self) -> impl Iterator<Item = &System> { self.systems.iter() }
}

pub trait ParseCtx {
	fn systems(&self) -> impl Iterator<Item = &System>;
	fn reporter(&self) -> &(impl Reporter + ?Sized);
}

pub async fn parse_items(
	ctx: &impl ParseCtx,
	path: Substack<'_, Tok<String>>,
	items: ParsSnippet<'_>,
) -> OrcRes<Vec<Item>> {
	let lines = line_items(items).await;
	let line_res =
		join_all(lines.into_iter().map(|p| parse_item(ctx, path.clone(), p.output, p.tail))).await;
	Ok(line_res.into_iter().flat_map(|l| l.ok().into_iter().flatten()).collect())
}

pub async fn parse_item(
	ctx: &impl ParseCtx,
	path: Substack<'_, Tok<String>>,
	comments: Vec<Comment>,
	item: ParsSnippet<'_>,
) -> OrcRes<Vec<Item>> {
	match item.pop_front() {
		Some((TokTree { tok: Token::Name(n), .. }, postdisc)) => match n {
			n if *n == item.i().i("export").await => match try_pop_no_fluff(postdisc).await? {
				Parsed { output: TokTree { tok: Token::Name(n), .. }, tail } =>
					parse_exportable_item(ctx, path, comments, true, n.clone(), tail).await,
				Parsed { output: TokTree { tok: Token::NS, .. }, tail } => {
					let Parsed { output: exports, tail } = parse_multiname(ctx.reporter(), tail).await?;
					let mut ok = Vec::new();
					for (e, pos) in exports {
						match (&e.path[..], e.name) {
							([], Some(n)) =>
								ok.push(Item { comments: comments.clone(), pos, kind: ItemKind::Export(n) }),
							(_, Some(_)) => ctx.reporter().report(mk_err(
								tail.i().i("Compound export").await,
								"Cannot export compound names (names containing the :: separator)",
								[pos.into()],
							)),
							(_, None) => ctx.reporter().report(mk_err(
								tail.i().i("Wildcard export").await,
								"Exports cannot contain the globstar *",
								[pos.into()],
							)),
						}
					}
					expect_end(tail).await?;
					Ok(ok)
				},
				Parsed { output, tail } => Err(mk_errv(
					tail.i().i("Malformed export").await,
					"`export` can either prefix other lines or list names inside ::( ) or ::[ ]",
					[Pos::Range(output.range.clone()).into()],
				)),
			},
			n if *n == item.i().i("import").await => parse_import(ctx, postdisc).await.map(|v| {
				Vec::from_iter(v.into_iter().map(|(t, pos)| Item {
					comments: comments.clone(),
					pos,
					kind: ItemKind::Import(t),
				}))
			}),
			n => parse_exportable_item(ctx, path, comments, false, n.clone(), postdisc).await,
		},
		Some(_) => Err(mk_errv(
			item.i().i("Expected a line type").await,
			"All lines must begin with a keyword",
			[Pos::Range(item.pos()).into()],
		)),
		None => unreachable!("These lines are filtered and aggregated in earlier stages"),
	}
}

pub async fn parse_import(
	ctx: &impl ParseCtx,
	tail: ParsSnippet<'_>,
) -> OrcRes<Vec<(Import, Pos)>> {
	let Parsed { output: imports, tail } = parse_multiname(ctx.reporter(), tail).await?;
	expect_end(tail).await?;
	Ok(imports)
}

pub async fn parse_exportable_item(
	ctx: &impl ParseCtx,
	path: Substack<'_, Tok<String>>,
	comments: Vec<Comment>,
	exported: bool,
	discr: Tok<String>,
	tail: ParsSnippet<'_>,
) -> OrcRes<Vec<Item>> {
	let kind = if discr == tail.i().i("mod").await {
		let (name, body) = parse_module(ctx, path, tail).await?;
		ItemKind::Member(Member::new(name, MemberKind::Mod(body)))
	} else if discr == tail.i().i("const").await {
		let (name, val) = parse_const(tail, path.clone()).await?;
		let locator = CodeLocator::to_const(tail.i().i(&path.push(name.clone()).unreverse()).await);
		ItemKind::Member(Member::new(name, MemberKind::Const(Code::from_code(locator, val))))
	} else if let Some(sys) = ctx.systems().find(|s| s.can_parse(discr.clone())) {
		let line = sys.parse(tail.to_vec(), exported, comments).await?;
		return parse_items(ctx, path, Snippet::new(tail.prev(), &line, tail.i())).await;
	} else {
		let ext_lines = ctx.systems().flat_map(System::line_types).join(", ");
		return Err(mk_errv(
			tail.i().i("Unrecognized line type").await,
			format!("Line types are: const, mod, macro, grammar, {ext_lines}"),
			[Pos::Range(tail.prev().range.clone()).into()],
		));
	};
	Ok(vec![Item { comments, pos: Pos::Range(tail.pos()), kind }])
}

pub async fn parse_module(
	ctx: &impl ParseCtx,
	path: Substack<'_, Tok<String>>,
	tail: ParsSnippet<'_>,
) -> OrcRes<(Tok<String>, Module)> {
	let (name, tail) = match try_pop_no_fluff(tail).await? {
		Parsed { output: TokTree { tok: Token::Name(n), .. }, tail } => (n.clone(), tail),
		Parsed { output, .. } => {
			return Err(mk_errv(
				tail.i().i("Missing module name").await,
				format!("A name was expected, {} was found", tail.fmt(output).await),
				[Pos::Range(output.range.clone()).into()],
			));
		},
	};
	let Parsed { output, tail: surplus } = try_pop_no_fluff(tail).await?;
	expect_end(surplus).await?;
	let Some(body) = output.as_s(Paren::Round, tail.i()) else {
		return Err(mk_errv(
			tail.i().i("Expected module body").await,
			format!("A ( block ) was expected, {} was found", tail.fmt(output).await),
			[Pos::Range(output.range.clone()).into()],
		));
	};
	let path = path.push(name.clone());
	Ok((name, Module::new(parse_items(ctx, path, body).await?)))
}

pub async fn parse_const(
	tail: ParsSnippet<'_>,
	path: Substack<'_, Tok<String>>,
) -> OrcRes<(Tok<String>, Vec<MacTree>)> {
	let Parsed { output, tail } = try_pop_no_fluff(tail).await?;
	let Some(name) = output.as_name() else {
		return Err(mk_errv(
			tail.i().i("Missing module name").await,
			format!("A name was expected, {} was found", tail.fmt(output).await),
			[Pos::Range(output.range.clone()).into()],
		));
	};
	let Parsed { output, tail } = try_pop_no_fluff(tail).await?;
	if !output.is_kw(tail.i().i("=").await) {
		return Err(mk_errv(
			tail.i().i("Missing = separator").await,
			format!("Expected = , found {}", tail.fmt(output).await),
			[Pos::Range(output.range.clone()).into()],
		));
	}
	try_pop_no_fluff(tail).await?;
	Ok((name, parse_mtree(tail, path).await?))
}

pub async fn parse_mtree(
	mut snip: ParsSnippet<'_>,
	path: Substack<'_, Tok<String>>,
) -> OrcRes<Vec<MacTree>> {
	let mut mtreev = Vec::new();
	while let Some((ttree, tail)) = snip.pop_front() {
		snip = tail;
		let (range, tok, tail) = match &ttree.tok {
			Token::S(p, b) => {
				let b = parse_mtree(Snippet::new(ttree, b, snip.i()), path.clone()).boxed_local().await?;
				(ttree.range.clone(), MTok::S(*p, b), tail)
			},
			Token::Reference(name) => (ttree.range.clone(), MTok::Name(name.clone()), tail),
			Token::Name(tok) => {
				let mut segments = path.unreverse();
				segments.push(tok.clone());
				let mut end = ttree.range.end;
				while let Some((TokTree { tok: Token::NS, .. }, tail)) = snip.pop_front() {
					let Parsed { output, tail } = try_pop_no_fluff(tail).await?;
					let Some(seg) = output.as_name() else {
						return Err(mk_errv(
							tail.i().i("Namespaced name interrupted").await,
							"In expression context, :: must always be followed by a name.\n\
											::() is permitted only in import and export items",
							[Pos::Range(output.range.clone()).into()],
						));
					};
					segments.push(seg);
					snip = tail;
					end = output.range.end;
				}
				(ttree.range.start..end, MTok::Name(Sym::new(segments, snip.i()).await.unwrap()), snip)
			},
			Token::NS => {
				return Err(mk_errv(
					tail.i().i("Unexpected :: in expression").await,
					":: can only follow a name",
					[Pos::Range(ttree.range.clone()).into()],
				));
			},
			Token::Ph(ph) => (ttree.range.clone(), MTok::Ph(ph.clone()), tail),
			Token::Macro(_) => {
				return Err(mk_errv(
					tail.i().i("Invalid keyword in expression").await,
					"Expressions cannot use `macro` as a name.",
					[Pos::Range(ttree.range.clone()).into()],
				));
			},
			Token::Atom(a) => (ttree.range.clone(), MTok::Atom(a.clone()), tail),
			Token::BR | Token::Comment(_) => continue,
			Token::Bottom(e) => return Err(e.clone()),
			Token::LambdaHead(arg) => (
				ttree.range.start..snip.pos().end,
				MTok::Lambda(
					parse_mtree(Snippet::new(ttree, arg, snip.i()), path.clone()).boxed_local().await?,
					parse_mtree(tail, path.clone()).boxed_local().await?,
				),
				Snippet::new(ttree, &[], snip.i()),
			),
			Token::Slot(_) | Token::X(_) =>
				panic!("Did not expect {} in parsed token tree", tail.fmt(ttree).await),
		};
		mtreev.push(MTree { pos: Pos::Range(range.clone()), tok: Rc::new(tok) });
		snip = tail;
	}
	Ok(mtreev)
}

pub async fn parse_macro(
	tail: ParsSnippet<'_>,
	macro_i: u16,
	path: Substack<'_, Tok<String>>,
) -> OrcRes<Vec<Rule>> {
	let (surplus, prev, block) = match try_pop_no_fluff(tail).await? {
		Parsed { tail, output: o @ TokTree { tok: Token::S(Paren::Round, b), .. } } => (tail, o, b),
		Parsed { output, .. } => {
			return Err(mk_errv(
				tail.i().i("m").await,
				"Macro blocks must either start with a block or a ..$:number",
				[Pos::Range(output.range.clone()).into()],
			));
		},
	};
	expect_end(surplus).await?;
	let mut errors = Vec::new();
	let mut rules = Vec::new();
	for (i, item) in line_items(Snippet::new(prev, block, tail.i())).await.into_iter().enumerate() {
		let Parsed { tail, output } = try_pop_no_fluff(item.tail).await?;
		if !output.is_kw(tail.i().i("rule").await) {
			errors.extend(mk_errv(
				tail.i().i("non-rule in macro").await,
				format!("Expected `rule`, got {}", tail.fmt(output).await),
				[Pos::Range(output.range.clone()).into()],
			));
			continue;
		};
		let arrow = tail.i().i("=>").await;
		let (pat, body) = match tail.split_once(|t| t.is_kw(arrow.clone())) {
			Some((a, b)) => (a, b),
			None => {
				errors.extend(mk_errv(
					tail.i().i("no => in macro rule").await,
					"The pattern and body of a rule must be separated by a =>",
					[Pos::Range(tail.pos()).into()],
				));
				continue;
			},
		};
		rules.push(Rule {
			comments: item.output,
			pos: Pos::Range(tail.pos()),
			pattern: parse_mtree(pat, path.clone()).await?,
			kind: RuleKind::Native(Code::from_code(
				CodeLocator::to_rule(tail.i().i(&path.unreverse()).await, macro_i, i as u16),
				parse_mtree(body, path.clone()).await?,
			)),
		})
	}
	if let Ok(e) = OrcErrv::new(errors) { Err(e) } else { Ok(rules) }
}
