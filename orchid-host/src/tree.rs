use std::fmt::Debug;
use std::sync::{Mutex, OnceLock};

use async_stream::stream;
use futures::{FutureExt, StreamExt};
use itertools::Itertools;
use never::Never;
use orchid_base::clone;
use orchid_base::error::OrcRes;
use orchid_base::interner::Tok;
use orchid_base::location::Pos;
use orchid_base::macros::mtreev_from_api;
use orchid_base::name::Sym;
use orchid_base::parse::{Comment, Import};
use orchid_base::tree::{AtomRepr, TokTree, Token};
use ordered_float::NotNan;

use crate::api;
use crate::atom::AtomHand;
use crate::expr::Expr;
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
						move |a| {
							clone!(pos, sys);
							Box::pin(async move {
								MacTok::Atom(AtomHand::from_api(a, pos.clone(), &mut sys.ctx().clone()).await)
							})
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

pub struct Member {
	pub name: Tok<String>,
	pub kind: OnceLock<MemberKind>,
	pub lazy: Mutex<Option<LazyMemberHandle>>,
}
impl Member {
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
		Member { name, kind: OnceLock::from(kind), lazy: Mutex::default() }
	}
	pub fn new(name: Tok<String>, kind: MemberKind) -> Self {
		Member { name, kind: OnceLock::from(kind), lazy: Mutex::default() }
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

#[derive(Debug)]
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
	pub async fn from_api(m: api::Module, path: &mut Vec<Tok<String>>, sys: &System) -> Self {
		Self::new(
			stream! { for item in m.items { yield Item::from_api(item, path, sys).boxed_local().await } }
				.collect::<Vec<_>>()
				.await,
		)
	}
}

pub struct LazyMemberHandle(api::TreeId, System, Vec<Tok<String>>);
impl LazyMemberHandle {
	pub async fn run(self) -> OrcRes<MemberKind> {
		match self.1.get_tree(self.0).await {
			api::MemberKind::Const(c) => Ok(MemberKind::Const(Code {
				bytecode: Expr::from_api(&c, &mut self.1.ext().clone()).await.into(),
				locator: CodeLocator { steps: self.1.ctx().i.i(&self.2).await, rule_loc: None },
				source: None,
			})),
			api::MemberKind::Module(m) =>
				Ok(MemberKind::Mod(Module::from_api(m, &mut { self.2 }, &self.1).await)),
			api::MemberKind::Lazy(id) => Self(id, self.1, self.2).run().boxed_local().await,
		}
	}
	pub fn into_member(self, name: Tok<String>) -> Member {
		Member { name, kind: OnceLock::new(), lazy: Mutex::new(Some(self)) }
	}
}

#[derive(Debug)]
pub struct Rule {
	pub pos: Pos,
	pub comments: Vec<Comment>,
	pub pattern: Vec<MacTree>,
	pub kind: RuleKind,
}

#[derive(Debug)]
pub enum RuleKind {
	Remote(System, api::MacroId),
	Native(Code),
}

#[derive(Debug)]
pub struct Code {
	locator: CodeLocator,
	source: Option<Vec<ParsTokTree>>,
	bytecode: OnceLock<Expr>,
}
impl Code {
	pub fn from_expr(locator: CodeLocator, expr: Expr) -> Self {
		Self { locator, source: None, bytecode: expr.into() }
	}
	pub fn from_code(locator: CodeLocator, code: Vec<ParsTokTree>) -> Self {
		Self { locator, source: Some(code), bytecode: OnceLock::new() }
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
