use std::fmt::Debug;
use std::rc::Rc;

use async_once_cell::OnceCell;
use async_std::sync::{Mutex, RwLock};
use async_stream::stream;
use futures::future::join_all;
use futures::{FutureExt, StreamExt};
use itertools::Itertools;
use never::Never;
use orchid_base::error::{OrcRes, mk_errv};
use orchid_base::format::{FmtCtx, FmtUnit, Format, Variants};
use orchid_base::interner::Tok;
use orchid_base::location::Pos;
use orchid_base::macros::{mtreev_fmt, mtreev_from_api};
use orchid_base::name::{NameLike, Sym};
use orchid_base::parse::{Comment, Import};
use orchid_base::tree::{AtomRepr, TokTree, Token};
use orchid_base::{clone, tl_cache};
use ordered_float::NotNan;
use substack::Substack;

use crate::api;
use crate::atom::AtomHand;
use crate::ctx::Ctx;
use crate::expr::{Expr, mtreev_to_expr};
use crate::macros::{MacTok, MacTree};
use crate::system::System;

pub type ParsTokTree = TokTree<'static, AtomHand, Never>;
pub type ParsTok = Token<'static, AtomHand, Never>;

#[derive(Debug)]
pub struct Item {
	pub pos: Pos,
	pub comments: Vec<Comment>,
	pub kind: ItemKind,
}

#[derive(Debug)]
pub enum ItemKind {
	Member(Member),
	Export(Tok<String>),
	Import(Import),
	Macro(Option<NotNan<f64>>, Vec<Rule>),
}
impl ItemKind {
	pub fn at(self, pos: Pos) -> Item { Item { comments: vec![], pos, kind: self } }
}

impl Item {
	pub async fn from_api(tree: api::Item, path: &mut Vec<Tok<String>>, sys: &System) -> Self {
		let kind = match tree.kind {
			api::ItemKind::Member(m) => ItemKind::Member(Member::from_api(m, path, sys).await),
			api::ItemKind::Import(name) => ItemKind::Import(Import {
				path: Sym::from_api(name, &sys.ctx().i).await.iter().collect(),
				name: None,
			}),
			api::ItemKind::Export(e) => ItemKind::Export(Tok::from_api(e, &sys.ctx().i).await),
			api::ItemKind::Macro(macro_block) => {
				let mut rules = Vec::new();
				for rule in macro_block.rules {
					let mut comments = Vec::new();
					for comment in rule.comments {
						comments.push(Comment::from_api(&comment, &sys.ctx().i).await);
					}
					let pos = Pos::from_api(&rule.location, &sys.ctx().i).await;
					let pattern = mtreev_from_api(&rule.pattern, &sys.ctx().i, &mut {
						clone!(pos, sys);
						async move |a| {
							MacTok::Atom(AtomHand::from_api(a, pos.clone(), &mut sys.ctx().clone()).await)
						}
					})
					.await;
					rules.push(Rule { pos, pattern, kind: RuleKind::Remote(sys.clone(), rule.id), comments });
				}
				ItemKind::Macro(macro_block.priority, rules)
			},
		};
		let mut comments = Vec::new();
		for comment in tree.comments.iter() {
			comments.push(Comment::from_api(comment, &sys.ctx().i).await)
		}
		Self { pos: Pos::from_api(&tree.location, &sys.ctx().i).await, comments, kind }
	}
}
impl Format for Item {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		let comment_text = self.comments.iter().join("\n");
		let item_text = match &self.kind {
			ItemKind::Import(i) => format!("import {i}").into(),
			ItemKind::Export(e) => format!("export {e}").into(),
			ItemKind::Macro(None, rules) =>
				tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("macro {{\n\t{0}\n}}")))
					.units([Variants::sequence(rules.len(), "\n", None)
						.units(join_all(rules.iter().map(|r| r.print(c))).await)]),
			ItemKind::Member(mem) => match mem.kind.get() {
				None => format!("lazy {}", mem.name).into(),
				Some(MemberKind::Const(val)) =>
					tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("const {0} = {1}")))
						.units([mem.name.rc().into(), val.print(c).await]),
				Some(MemberKind::Mod(module)) =>
					tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("module {0} {{\n\t{1}\n}}")))
						.units([mem.name.rc().into(), module.print(c).boxed_local().await]),
			},
			_ => panic!(),
		};
		tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("{0}\n{1}")))
			.units([comment_text.into(), item_text])
	}
}

pub struct Member {
	name: Tok<String>,
	kind: OnceCell<MemberKind>,
	lazy: Mutex<Option<LazyMemberHandle>>,
}
impl Member {
	pub fn name(&self) -> Tok<String> { self.name.clone() }
	pub async fn kind(&self) -> &MemberKind {
		(self.kind.get_or_init(async {
			let handle = self.lazy.lock().await.take().expect("Neither known nor lazy");
			handle.run().await
		}))
		.await
	}
	pub async fn kind_mut(&mut self) -> &mut MemberKind {
		self.kind().await;
		self.kind.get_mut().expect("kind() already filled the cell")
	}
	pub async fn from_api(api: api::Member, path: &mut Vec<Tok<String>>, sys: &System) -> Self {
		path.push(Tok::from_api(api.name, &sys.ctx().i).await);
		let kind = match api.kind {
			api::MemberKind::Lazy(id) => {
				let handle = LazyMemberHandle(id, sys.clone(), path.clone());
				return handle.into_member(path.pop().unwrap());
			},
			api::MemberKind::Const(c) => MemberKind::Const(Code::from_expr(
				CodeLocator::to_const(sys.ctx().i.i(&*path).await),
				Expr::from_api(&c, &mut sys.ext().clone()).await,
			)),
			api::MemberKind::Module(m) => MemberKind::Mod(Module::from_api(m, path, sys).await),
		};
		let name = path.pop().unwrap();
		Member { name, kind: OnceCell::from(kind), lazy: Mutex::default() }
	}
	pub fn new(name: Tok<String>, kind: MemberKind) -> Self {
		Member { name, kind: OnceCell::from(kind), lazy: Mutex::default() }
	}
}
impl Debug for Member {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Member")
			.field("name", &self.name)
			.field("kind", &self.kind)
			.finish_non_exhaustive()
	}
}

#[derive(Debug)]
pub enum MemberKind {
	Const(Code),
	Mod(Module),
}

#[derive(Debug, Default)]
pub struct Module {
	pub imports: Vec<Sym>,
	pub exports: Vec<Tok<String>>,
	pub items: Vec<Item>,
}
impl Module {
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
	pub fn merge(&mut self, other: Module) {
		let mut swap = Module::default();
		std::mem::swap(self, &mut swap);
		*self = Module::new(swap.items.into_iter().chain(other.items))
	}
	pub async fn from_api(m: api::Module, path: &mut Vec<Tok<String>>, sys: &System) -> Self {
		Self::new(
			stream! { for item in m.items { yield Item::from_api(item, path, sys).boxed_local().await } }
				.collect::<Vec<_>>()
				.await,
		)
	}
	pub async fn walk(
		&self,
		allow_private: bool,
		path: impl IntoIterator<Item = Tok<String>>,
	) -> Result<&Module, WalkError> {
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
			match member.kind().await {
				MemberKind::Const(_) => return Err(WalkError { pos, kind: WalkErrorKind::Constant }),
				MemberKind::Mod(m) => cur = m,
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
impl Format for Module {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		let import_str = self.imports.iter().map(|i| format!("import {i}")).join("\n");
		let head_str = format!("{import_str}\nexport ::({})\n", self.exports.iter().join(", "));
		Variants::sequence(self.items.len() + 1, "\n", None).units(
			[head_str.into()].into_iter().chain(join_all(self.items.iter().map(|i| i.print(c))).await),
		)
	}
}

pub struct LazyMemberHandle(api::TreeId, System, Vec<Tok<String>>);
impl LazyMemberHandle {
	pub async fn run(self) -> MemberKind {
		match self.1.get_tree(self.0).await {
			api::MemberKind::Const(c) => MemberKind::Const(Code {
				bytecode: Expr::from_api(&c, &mut self.1.ext().clone()).await.into(),
				locator: CodeLocator { steps: self.1.ctx().i.i(&self.2).await, rule_loc: None },
				source: None,
			}),
			api::MemberKind::Module(m) =>
				MemberKind::Mod(Module::from_api(m, &mut { self.2 }, &self.1).await),
			api::MemberKind::Lazy(id) => Self(id, self.1, self.2).run().boxed_local().await,
		}
	}
	pub fn into_member(self, name: Tok<String>) -> Member {
		Member { name, kind: OnceCell::new(), lazy: Mutex::new(Some(self)) }
	}
}

#[derive(Debug)]
pub struct Rule {
	pub pos: Pos,
	pub comments: Vec<Comment>,
	pub pattern: Vec<MacTree>,
	pub kind: RuleKind,
}
impl Format for Rule {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		FmtUnit::new(
			tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("{0b}\n{1} => {2b}"))),
			[
				self.comments.iter().join("\n").into(),
				mtreev_fmt(&self.pattern, c).await,
				match &self.kind {
					RuleKind::Native(code) => code.print(c).await,
					RuleKind::Remote(sys, id) => FmtUnit::new(
						tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("{0} #{1}"))),
						[sys.print(c).await, format!("{id:?}").into()],
					),
				},
			],
		)
	}
}

#[derive(Debug)]
pub enum RuleKind {
	Remote(System, api::MacroId),
	Native(Code),
}

#[derive(Debug)]
pub struct Code {
	locator: CodeLocator,
	source: Option<Vec<MacTree>>,
	bytecode: OnceCell<Expr>,
}
impl Code {
	pub fn from_expr(locator: CodeLocator, expr: Expr) -> Self {
		Self { locator, source: None, bytecode: expr.into() }
	}
	pub fn from_code(locator: CodeLocator, code: Vec<MacTree>) -> Self {
		Self { locator, source: Some(code), bytecode: OnceCell::new() }
	}
	pub fn source(&self) -> Option<&Vec<MacTree>> { self.source.as_ref() }
	pub fn set_source(&mut self, source: Vec<MacTree>) {
		self.source = Some(source);
		self.bytecode = OnceCell::new();
	}
	pub async fn get_bytecode(&self, ctx: &Ctx) -> &Expr {
		(self.bytecode.get_or_init(async {
			let src = self.source.as_ref().expect("no bytecode or source");
			mtreev_to_expr(src, Substack::Bottom, ctx).await.at(Pos::None)
		}))
		.await
	}
}
impl Format for Code {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		if let Some(bc) = self.bytecode.get() {
			return bc.print(c).await;
		}
		if let Some(src) = &self.source {
			return mtreev_fmt(src, c).await;
		}
		panic!("Code must be initialized with at least one state")
	}
}

/// Selects a code element
///
/// Either the steps point to a constant and rule_loc is None, or the steps
/// point to a module and rule_loc selects a macro rule within that module
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct CodeLocator {
	steps: Tok<Vec<Tok<String>>>,
	/// Index of a macro block in the module demarked by the steps, and a rule in
	/// that macro
	rule_loc: Option<(u16, u16)>,
}
impl CodeLocator {
	pub fn to_const(steps: Tok<Vec<Tok<String>>>) -> Self { Self { steps, rule_loc: None } }
	pub fn to_rule(steps: Tok<Vec<Tok<String>>>, macro_i: u16, rule_i: u16) -> Self {
		Self { steps, rule_loc: Some((macro_i, rule_i)) }
	}
}

#[derive(Clone)]
pub struct Root(Rc<RwLock<Module>>);
impl Root {
	pub fn new(module: Module) -> Self { Self(Rc::new(RwLock::new(module))) }
	pub async fn get_const_value(&self, name: impl NameLike, pos: Pos, ctx: Ctx) -> OrcRes<Expr> {
		let (cn, mp) = name.split_last();
		let root_lock = self.0.read().await;
		let module = root_lock.walk(true, mp.iter().cloned()).await.unwrap();
		let member = (module.items.iter())
			.filter_map(|it| if let ItemKind::Member(m) = &it.kind { Some(m) } else { None })
			.find(|m| m.name() == cn);
		match member {
			None => Err(mk_errv(
				ctx.i.i("Constant does not exist").await,
				format!("{name} does not refer to a constant"),
				[pos.clone().into()],
			)),
			Some(mem) => match mem.kind().await {
				MemberKind::Mod(_) => Err(mk_errv(
					ctx.i.i("module used as constant").await,
					format!("{name} is a module, not a constant"),
					[pos.clone().into()],
				)),
				MemberKind::Const(c) => Ok((c.get_bytecode(&ctx).await).clone()),
			},
		}
	}
}
