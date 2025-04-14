use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;

use async_once_cell::OnceCell;
use async_std::sync::{Mutex, RwLock};
use async_stream::stream;
use futures::future::join_all;
use futures::{FutureExt, StreamExt};
use hashbrown::HashMap;
use itertools::Itertools;
use orchid_base::error::{OrcRes, mk_errv};
use orchid_base::format::{FmtCtx, FmtUnit, Format, Variants};
use orchid_base::interner::Tok;
use orchid_base::location::Pos;
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
		_: Pos,
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
		_: Pos,
		_: &orchid_base::interner::Interner,
	) -> Self {
		Expr::from_api(api, PathSetBuilder::new(), ctx).await
	}
	type ToApiCtx<'a> = ();
	async fn into_api(self, (): &mut Self::ToApiCtx<'_>) -> api::Expression {
		panic!("Failed to replace NewExpr before returning sublexer value")
	}
}

pub struct ParsedFromApiCx<'a> {
	pub sys: &'a System,
	pub consts: &'a mut HashMap<Sym, Expr>,
	pub path: Tok<Vec<Tok<String>>>,
}
impl<'a> ParsedFromApiCx<'a> {
	pub async fn push<'c>(&'c mut self, name: Tok<String>) -> ParsedFromApiCx<'c> {
		let path = self.sys.ctx().i.i(&self.path.iter().cloned().chain([name]).collect_vec()).await;
		ParsedFromApiCx { path, consts: &mut *self.consts, sys: self.sys }
	}
}

#[derive(Debug)]
pub struct Item {
	pub pos: Pos,
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
	pub fn at(self, pos: Pos) -> Item { Item { comments: vec![], pos, kind: self } }
}

impl Item {
	pub async fn from_api<'a>(tree: api::Item, ctx: &mut ParsedFromApiCx<'a>) -> Self {
		let kind = match tree.kind {
			api::ItemKind::Member(m) => ItemKind::Member(ParsedMember::from_api(m, ctx).await),
			api::ItemKind::Import(name) => ItemKind::Import(Import {
				path: Sym::from_api(name, &ctx.sys.ctx().i).await.iter().collect(),
				name: None,
			}),
			api::ItemKind::Export(e) => ItemKind::Export(Tok::from_api(e, &ctx.sys.ctx().i).await),
		};
		let mut comments = Vec::new();
		for comment in tree.comments.iter() {
			comments.push(Comment::from_api(comment, &ctx.sys.ctx().i).await)
		}
		Self { pos: Pos::from_api(&tree.location, &ctx.sys.ctx().i).await, comments, kind }
	}
}
impl Format for Item {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		let comment_text = self.comments.iter().join("\n");
		let item_text = match &self.kind {
			ItemKind::Import(i) => format!("import {i}").into(),
			ItemKind::Export(e) => format!("export {e}").into(),
			ItemKind::Member(mem) => match mem.kind.get() {
				None => format!("lazy {}", mem.name).into(),
				Some(ParsedMemberKind::Const) =>
					tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("const {0}")))
						.units([mem.name.rc().into()]),
				Some(ParsedMemberKind::Mod(module)) =>
					tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("module {0} {{\n\t{1}\n}}")))
						.units([mem.name.rc().into(), module.print(c).boxed_local().await]),
			},
		};
		tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("{0}\n{1}")))
			.units([comment_text.into(), item_text])
	}
}

pub struct ParsedMember {
	name: Tok<String>,
	full_name: Sym,
	kind: OnceCell<ParsedMemberKind>,
	lazy: Mutex<Option<LazyMemberHandle>>,
}
// TODO: this one should own but not execute the lazy handle.
// Lazy handles should run
// - in the tree converter function as needed to resolve imports
// - in the tree itself when a constant is loaded
// - when a different lazy subtree references them in a wildcard import and
//   forces the enumeration.
//
// do we actually need to allow wildcard imports in lazy trees? maybe a
// different kind of import is sufficient. Source code never becomes a lazy
// tree. What does?
// - Systems subtrees rarely reference each other at all. They can't use macros
//   and they usually point to constants with an embedded expr.
// - Compiled libraries on the long run. The code as written may reference
//   constants by indirect path. But this is actually the same as the above,
//   they also wouldn't use regular imports because they are distributed as
//   exprs.
//
// Everything is distributed either as source code or as exprs. Line parsers
// also operate on tokens.
//
// TODO: The trees produced by systems can be safely changed
// to the new kind of tree. This datastructure does not need to support the lazy
// handle.
impl ParsedMember {
	pub fn name(&self) -> Tok<String> { self.name.clone() }
	pub async fn kind(&self, consts: &mut HashMap<Sym, Expr>) -> &ParsedMemberKind {
		(self.kind.get_or_init(async {
			let handle = self.lazy.lock().await.take().expect("Neither known nor lazy");
			handle.run(consts).await
		}))
		.await
	}
	pub async fn kind_mut(&mut self, consts: &mut HashMap<Sym, Expr>) -> &mut ParsedMemberKind {
		self.kind(consts).await;
		self.kind.get_mut().expect("kind() already filled the cell")
	}
	pub async fn from_api<'a>(api: api::Member, ctx: &'_ mut ParsedFromApiCx<'a>) -> Self {
		let name = Tok::from_api(api.name, &ctx.sys.ctx().i).await;
		let mut ctx: ParsedFromApiCx<'_> = (&mut *ctx).push(name.clone()).await;
		let path_sym = Sym::from_tok(ctx.path.clone()).expect("We just pushed on to this");
		let kind = match api.kind {
			api::MemberKind::Lazy(id) => {
				let handle = LazyMemberHandle { id, sys: ctx.sys.clone(), path: path_sym.clone() };
				return handle.into_member(name.clone()).await;
			},
			api::MemberKind::Const(c) => {
				let mut pctx =
					ExprParseCtx { ctx: ctx.sys.ctx().clone(), exprs: ctx.sys.ext().exprs().clone() };
				let expr = Expr::from_api(&c, PathSetBuilder::new(), &mut pctx).await;
				ctx.consts.insert(path_sym.clone(), expr);
				ParsedMemberKind::Const
			},
			api::MemberKind::Module(m) =>
				ParsedMemberKind::Mod(ParsedModule::from_api(m, &mut ctx).await),
		};
		ParsedMember { name, full_name: path_sym, kind: OnceCell::from(kind), lazy: Mutex::default() }
	}
	pub fn new(name: Tok<String>, full_name: Sym, kind: ParsedMemberKind) -> Self {
		ParsedMember { name, full_name, kind: OnceCell::from(kind), lazy: Mutex::default() }
	}
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
	pub imports: Vec<Sym>,
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
		Self { imports: vec![], exports, items }
	}
	pub fn merge(&mut self, other: ParsedModule) {
		let mut swap = ParsedModule::default();
		std::mem::swap(self, &mut swap);
		*self = ParsedModule::new(swap.items.into_iter().chain(other.items))
	}
	pub async fn from_api<'a>(m: api::Module, ctx: &mut ParsedFromApiCx<'a>) -> Self {
		Self::new(
			stream! { for item in m.items { yield Item::from_api(item, ctx).boxed_local().await } }
				.collect::<Vec<_>>()
				.await,
		)
	}
	pub async fn walk<'a>(
		&self,
		allow_private: bool,
		path: impl IntoIterator<Item = Tok<String>>,
		consts: &mut HashMap<Sym, Expr>,
	) -> Result<&ParsedModule, WalkError> {
		let mut cur = self;
		for (pos, step) in path.into_iter().enumerate() {
			let Some(member) = (cur.items.iter())
				.filter_map(|it| if let ItemKind::Member(m) = &it.kind { Some(m) } else { None })
				.find(|m| m.name == step)
			else {
				return Err(WalkError { pos, kind: WalkErrorKind::Missing });
			};
			if !allow_private && !cur.exports.contains(&step) {
				return Err(WalkError { pos, kind: WalkErrorKind::Private });
			}
			match member.kind(consts).await {
				ParsedMemberKind::Const => return Err(WalkError { pos, kind: WalkErrorKind::Constant }),
				ParsedMemberKind::Mod(m) => cur = m,
			}
		}
		Ok(cur)
	}
}
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum WalkErrorKind {
	Missing,
	Private,
	Constant,
}
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct WalkError {
	pub pos: usize,
	pub kind: WalkErrorKind,
}
impl Format for ParsedModule {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		let import_str = self.imports.iter().map(|i| format!("import {i}")).join("\n");
		let head_str = format!("{import_str}\nexport ::({})\n", self.exports.iter().join(", "));
		Variants::sequence(self.items.len() + 1, "\n", None).units(
			[head_str.into()].into_iter().chain(join_all(self.items.iter().map(|i| i.print(c))).await),
		)
	}
}

pub struct LazyMemberHandle {
	id: api::TreeId,
	sys: System,
	path: Sym,
}
impl LazyMemberHandle {
	pub async fn run(self, consts: &mut HashMap<Sym, Expr>) -> ParsedMemberKind {
		match self.sys.get_tree(self.id).await {
			api::MemberKind::Const(c) => {
				let mut pctx =
					ExprParseCtx { ctx: self.sys.ctx().clone(), exprs: self.sys.ext().exprs().clone() };
				consts.insert(self.path, Expr::from_api(&c, PathSetBuilder::new(), &mut pctx).await);
				ParsedMemberKind::Const
			},
			api::MemberKind::Module(m) => ParsedMemberKind::Mod(
				ParsedModule::from_api(m, &mut ParsedFromApiCx {
					sys: &self.sys,
					consts,
					path: self.path.tok(),
				})
				.await,
			),
			api::MemberKind::Lazy(id) => Self { id, ..self }.run(consts).boxed_local().await,
		}
	}
	pub async fn into_member(self, name: Tok<String>) -> ParsedMember {
		ParsedMember {
			name,
			full_name: self.path.clone(),
			kind: OnceCell::new(),
			lazy: Mutex::new(Some(self)),
		}
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
		let consts_mut = &mut *self.consts.write().await;
		let module = self.tree.walk(true, mp.iter().cloned(), consts_mut).await.unwrap();
		let member = (module.items.iter())
			.filter_map(|it| if let ItemKind::Member(m) = &it.kind { Some(m) } else { None })
			.find(|m| m.name() == cn);
		match member {
			None => Err(mk_errv(
				ctx.i.i("Constant does not exist").await,
				format!("{name} does not refer to a constant"),
				[pos.clone().into()],
			)),
			Some(mem) => match mem.kind(consts_mut).await {
				ParsedMemberKind::Mod(_) => Err(mk_errv(
					ctx.i.i("module used as constant").await,
					format!("{name} is a module, not a constant"),
					[pos.clone().into()],
				)),
				ParsedMemberKind::Const => Ok(
					(consts_mut.get(&name).cloned())
						.expect("Tree says the path is correct but no value was found"),
				),
			},
		}
	}
}
