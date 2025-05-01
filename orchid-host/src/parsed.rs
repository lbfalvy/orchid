use std::fmt::Debug;
use std::rc::Rc;

use async_once_cell::OnceCell;
use async_std::sync::{Mutex, RwLock};
use async_stream::stream;
use futures::future::join_all;
use futures::{FutureExt, StreamExt};
use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use orchid_base::error::{OrcRes, mk_errv};
use orchid_base::format::{FmtCtx, FmtUnit, Format, Variants};
use orchid_base::interner::Tok;
use orchid_base::location::{Pos, SrcRange};
use orchid_base::name::{NameLike, Sym};
use orchid_base::parse::{Comment, Import};
use orchid_base::tl_cache;
use orchid_base::tree::{TokTree, Token, TokenVariant};

use crate::api;
use crate::ctx::Ctx;
use crate::expr::{Expr, ExprParseCtx, PathSetBuilder};
use crate::expr_store::ExprStore;
use crate::system::System;

pub type ParsTokTree = TokTree<Expr, Expr>;
pub type ParsTok = Token<Expr, Expr>;

impl TokenVariant<api::ExprTicket> for Expr {
	type ToApiCtx<'a> = ExprStore;
	async fn into_api(self, ctx: &mut Self::ToApiCtx<'_>) -> api::ExprTicket {
		ctx.give_expr(self.clone());
		self.id()
	}
	type FromApiCtx<'a> = ExprStore;
	async fn from_api(
		api: &api::ExprTicket,
		ctx: &mut Self::FromApiCtx<'_>,
		_: SrcRange,
		_: &orchid_base::interner::Interner,
	) -> Self {
		let expr = ctx.get_expr(*api).expect("Dangling expr");
		ctx.take_expr(*api);
		expr
	}
}

impl TokenVariant<api::Expression> for Expr {
	type FromApiCtx<'a> = ExprParseCtx;
	async fn from_api(
		api: &api::Expression,
		ctx: &mut Self::FromApiCtx<'_>,
		_: SrcRange,
		_: &orchid_base::interner::Interner,
	) -> Self {
		Expr::from_api(api, PathSetBuilder::new(), ctx).await
	}
	type ToApiCtx<'a> = ();
	async fn into_api(self, (): &mut Self::ToApiCtx<'_>) -> api::Expression {
		panic!("Failed to replace NewExpr before returning sublexer value")
	}
}

#[derive(Debug)]
pub struct Item {
	pub sr: SrcRange,
	pub comments: Vec<Comment>,
	pub kind: ItemKind,
}

#[derive(Debug)]
pub enum ItemKind {
	Member(ParsedMember),
	Export(Tok<String>),
	Import(Import),
}
impl ItemKind {
	pub fn at(self, sr: SrcRange) -> Item { Item { comments: vec![], sr, kind: self } }
}

impl Format for Item {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		let comment_text = self.comments.iter().join("\n");
		let item_text = match &self.kind {
			ItemKind::Import(i) => format!("import {i}").into(),
			ItemKind::Export(e) => format!("export {e}").into(),
			ItemKind::Member(mem) => match &mem.kind {
				ParsedMemberKind::Const =>
					tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("const {0}")))
						.units([mem.name.rc().into()]),
				ParsedMemberKind::Mod(module) =>
					tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("module {0} {{\n\t{1}\n}}")))
						.units([mem.name.rc().into(), module.print(c).boxed_local().await]),
			},
		};
		tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("{0}\n{1}")))
			.units([comment_text.into(), item_text])
	}
}

pub struct ParsedMember {
	pub name: Tok<String>,
	pub full_name: Sym,
	pub kind: ParsedMemberKind,
}
impl ParsedMember {
	pub fn name(&self) -> Tok<String> { self.name.clone() }
}
impl Debug for ParsedMember {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Member")
			.field("name", &self.name)
			.field("kind", &self.kind)
			.finish_non_exhaustive()
	}
}

#[derive(Debug)]
pub enum ParsedMemberKind {
	Const,
	Mod(ParsedModule),
}

#[derive(Debug, Default)]
pub struct ParsedModule {
	pub exports: Vec<Tok<String>>,
	pub items: Vec<Item>,
}
impl ParsedModule {
	pub fn new(items: impl IntoIterator<Item = Item>) -> Self {
		let items = items.into_iter().collect_vec();
		let exports = (items.iter())
			.filter_map(|i| match &i.kind {
				ItemKind::Export(e) => Some(e.clone()),
				_ => None,
			})
			.collect_vec();
		Self { exports, items }
	}
	pub fn merge(&mut self, other: ParsedModule) {
		let mut swap = ParsedModule::default();
		std::mem::swap(self, &mut swap);
		*self = ParsedModule::new(swap.items.into_iter().chain(other.items))
	}
	pub fn get_imports(&self) -> impl IntoIterator<Item = &Import> {
		(self.items.iter())
			.filter_map(|it| if let ItemKind::Import(i) = &it.kind { Some(i) } else { None })
	}
}
impl Tree for ParsedModule {
	type Ctx = ();
	async fn walk<I: IntoIterator<Item = Tok<String>>>(
		&self,
		public_only: bool,
		path: I,
		_ctx: &'_ mut Self::Ctx,
	) -> Result<&Self, WalkError> {
		let mut cur = self;
		for (pos, step) in path.into_iter().enumerate() {
			let Some(member) = (cur.items.iter())
				.filter_map(|it| if let ItemKind::Member(m) = &it.kind { Some(m) } else { None })
				.find(|m| m.name == step)
			else {
				return Err(WalkError { pos, kind: WalkErrorKind::Missing });
			};
			if public_only && !cur.exports.contains(&step) {
				return Err(WalkError { pos, kind: WalkErrorKind::Private });
			}
			match &member.kind {
				ParsedMemberKind::Const => return Err(WalkError { pos, kind: WalkErrorKind::Constant }),
				ParsedMemberKind::Mod(m) => cur = m,
			}
		}
		Ok(cur)
	}
	fn children(&self, public_only: bool) -> HashSet<Tok<String>> {
		let mut public: HashSet<_> = self.exports.iter().cloned().collect();
		if !public_only {
			public.extend(
				(self.items.iter())
					.filter_map(
						|it| if let ItemKind::Member(mem) = &it.kind { Some(&mem.name) } else { None },
					)
					.cloned(),
			)
		}
		public
	}
}
impl Format for ParsedModule {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		let head_str = format!("export ::({})\n", self.exports.iter().join(", "));
		Variants::sequence(self.items.len() + 1, "\n", None).units(
			[head_str.into()].into_iter().chain(join_all(self.items.iter().map(|i| i.print(c))).await),
		)
	}
}

/// TODO:
///
/// idea, does the host need an IR here or can we figure out a way to transcribe
/// these? Should we spin off a new stage for value parsing so that ParsTokTree
/// doesn't appear in the interpreter's ingress?
pub struct Const {
	pub source: Option<Vec<ParsTokTree>>,
}

/// Selects a code element
///
/// Either the steps point to a constant and rule_loc is None, or the steps
/// point to a module and rule_loc selects a macro rule within that module
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ConstPath {
	steps: Tok<Vec<Tok<String>>>,
}
impl ConstPath {
	pub fn to_const(steps: Tok<Vec<Tok<String>>>) -> Self { Self { steps } }
}

#[derive(Clone)]
pub struct Root {
	tree: Rc<ParsedModule>,
	consts: Rc<RwLock<HashMap<Sym, Expr>>>,
}
impl Root {
	pub fn new(module: ParsedModule, consts: HashMap<Sym, Expr>) -> Self {
		Self { tree: Rc::new(module), consts: Rc::new(RwLock::new(consts)) }
	}
	pub async fn get_const_value(&self, name: Sym, pos: Pos, ctx: Ctx) -> OrcRes<Expr> {
		if let Some(val) = self.consts.read().await.get(&name) {
			return Ok(val.clone());
		}
		let (cn, mp) = name.split_last();
		let module = self.tree.walk(false, mp.iter().cloned(), &mut ()).await.unwrap();
		let member = (module.items.iter())
			.filter_map(|it| if let ItemKind::Member(m) = &it.kind { Some(m) } else { None })
			.find(|m| m.name() == cn);
		match member {
			None => Err(mk_errv(
				ctx.i.i("Constant does not exist").await,
				format!("{name} does not refer to a constant"),
				[pos.clone().into()],
			)),
			Some(mem) => match &mem.kind {
				ParsedMemberKind::Mod(_) => Err(mk_errv(
					ctx.i.i("module used as constant").await,
					format!("{name} is a module, not a constant"),
					[pos.clone().into()],
				)),
				ParsedMemberKind::Const => Ok(
					(self.consts.read().await.get(&name).cloned())
						.expect("Tree says the path is correct but no value was found"),
				),
			},
		}
	}
}
