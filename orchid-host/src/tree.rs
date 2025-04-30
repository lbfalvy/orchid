use std::cell::RefCell;
use std::rc::{Rc, Weak};

use async_once_cell::OnceCell;
use async_stream::stream;
use futures::StreamExt;
use hashbrown::HashMap;
use orchid_base::interner::Tok;
use orchid_base::name::Sym;

use crate::api;
use crate::ctx::Ctx;
use crate::dealias::absolute_path;
use crate::expr::Expr;
use crate::parsed::{ParsedMemberKind, ParsedModule};
use crate::system::System;

pub struct Tree(Rc<RefCell<Module>>);

pub struct WeakTree(Weak<RefCell<Module>>);

pub struct Module {
	pub members: HashMap<Tok<String>, Rc<Member>>,
}
impl Module {
	pub async fn from_api(
		api: api::Module,
		consts: &mut HashMap<Sym, Expr>,
		sys: System,
		path: &mut Vec<Tok<String>>
	) -> Self {
		let mut members = HashMap::new();
		for mem in api.members {
			let (lazy, kind) = match mem.kind {
				orchid_api::MemberKind::Lazy(id) => (Some(LazyMemberHandle{ id, sys: sys.clone(), path:  }))
			}
			members.insert(sys.ctx().i.ex(mem.name).await, member);
		}
		Self { members }
	}
	async fn walk(&self, mut path: impl Iterator<Item = Tok<String>>, ) -> &Self { todo!()}
	async fn from_parsed(
		parsed: &ParsedModule,
		path: Sym,
		parsed_root_path: Sym,
		parsed_root: &ParsedModule,
		root: &Module,
		preload: &mut HashMap<Sym, Module>,
	) -> Self {
		let mut imported_names = Vec::new();
		for import in parsed.get_imports() {
			if let Some(n) = import.name.clone() {
				imported_names.push(n);
				continue;
			}
			// the path in a wildcard import has to be a module
			if import.path.is_empty() {
				panic!("Imported root")
			}
			if let Some(subpath) = import.path.strip_prefix(&parsed_root_path) {
				let abs = absolute_path(&path, subpath);
				// path is in parsed_root
			} else {
				// path is in root 
			}
		}
		todo!()
	}
}

pub struct Member {
	pub public: bool,
	pub root: WeakTree,
	pub canonical_path: Sym,
	pub lazy: RefCell<Option<(LazyMemberHandle, Rc<ParsedModule>)>>,
	pub kind: OnceCell<MemberKind>,
}
impl Member {
	pub async fn kind_mut(&mut self, consts: &mut HashMap<Sym, Expr>) -> &mut MemberKind {
		self.kind(consts).await;
		self.kind.get_mut().expect("Thhe above line should have initialized it")
	}
	pub async fn kind(&self, consts: &mut HashMap<Sym, Expr>) -> &MemberKind {
		(self.kind.get_or_init(async {
			let (handle, root) =
				self.lazy.borrow_mut().take().expect("If kind is uninit, lazy must be Some");
			let parsed = handle.run(consts).await;
			MemberKind::from_parsed(&parsed, &root).await
		}))
		.await
	}
}

pub enum MemberKind {
	Const,
	Module(Module),
}
impl MemberKind {
	async fn from_parsed(parsed: &ParsedMemberKind, root: &ParsedModule) -> Self {
		match parsed {
			ParsedMemberKind::Const => MemberKind::Const,
			ParsedMemberKind::Mod(m) => MemberKind::Module(Module::from_parsed(m, root).await),
		}
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
	pub async fn into_member(self, public: bool, name: Tok<String>) -> Member {
		Member {
			name,
			public,
			canonical_path: self.path.clone(),
			kind: OnceCell::new(),
			lazy: Mutex::new(Some(self)),
		}
	}
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
