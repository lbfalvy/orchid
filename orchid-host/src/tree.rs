use std::cell::RefCell;
use std::rc::{Rc, Weak};

use async_once_cell::OnceCell;
use futures::FutureExt;
use hashbrown::HashMap;
use itertools::Itertools;
use orchid_base::error::{OrcRes, Reporter};
use orchid_base::interner::Tok;
use orchid_base::name::{Sym, VPath};

use crate::api;
use crate::ctx::Ctx;
use crate::dealias::{DealiasCtx, absolute_path, resolv_glob};
use crate::expr::{Expr, ExprParseCtx, PathSetBuilder};
use crate::parsed::{ParsedMemberKind, ParsedModule, Tree, WalkError, WalkErrorKind};
use crate::system::System;

pub struct Tree(Rc<RefCell<Module>>);

pub struct WeakTree(Weak<RefCell<Module>>);

pub struct TreeFromApiCtx<'a> {
	pub sys: &'a System,
	pub consts: &'a mut HashMap<Sym, Expr>,
	pub path: Tok<Vec<Tok<String>>>,
}
impl<'a> TreeFromApiCtx<'a> {
	pub async fn push<'c>(&'c mut self, name: Tok<String>) -> TreeFromApiCtx<'c> {
		let path = self.sys.ctx().i.i(&self.path.iter().cloned().chain([name]).collect_vec()).await;
		TreeFromApiCtx { path, consts: &mut *self.consts, sys: self.sys }
	}
}

pub struct Module {
	pub members: HashMap<Tok<String>, Rc<Member>>,
}
impl Module {
	pub async fn from_api(api: api::Module, ctx: &mut TreeFromApiCtx<'_>) -> Self {
		let mut members = HashMap::new();
		for mem in api.members {
			let mem_name = ctx.sys.i().ex(mem.name).await;
			let vname = VPath::new(ctx.path.iter().cloned()).name_with_suffix(mem_name.clone());
			let name = vname.to_sym(ctx.sys.i()).await;
			let (lazy, kind) = match mem.kind {
				api::MemberKind::Lazy(id) =>
					(Some(LazyMemberHandle { id, sys: ctx.sys.clone(), path: name.clone() }), None),
				api::MemberKind::Const(val) => {
					let mut expr_ctx =
						ExprParseCtx { ctx: ctx.sys.ctx().clone(), exprs: ctx.sys.ext().exprs().clone() };
					let expr = Expr::from_api(&val, PathSetBuilder::new(), &mut expr_ctx).await;
					ctx.consts.insert(name.clone(), expr);
					(None, Some(MemberKind::Const))
				},
				api::MemberKind::Module(m) => {
					let m = Self::from_api(m, &mut ctx.push(mem_name.clone()).await).boxed_local().await;
					(None, Some(MemberKind::Module(m)))
				},
				api::MemberKind::Import(import_path) =>
					(None, Some(MemberKind::Alias(Sym::from_api(import_path, ctx.sys.i()).await))),
			};
			members.insert(
				mem_name.clone(),
				Rc::new(Member {
					path: name.clone(),
					public: mem.exported,
					lazy: RefCell::new(lazy),
					kind: kind.map_or_else(OnceCell::new, OnceCell::from),
				}),
			);
		}
		Self { members }
	}
	async fn walk(&self, mut path: impl Iterator<Item = Tok<String>>) -> &Self { todo!() }
	async fn from_parsed(
		parsed: &ParsedModule,
		path: Sym,
		pars_root_path: Sym,
		pars_root: &ParsedModule,
		root: &Module,
		preload: &mut HashMap<Sym, Module>,
		ctx: &Ctx,
		rep: &Reporter,
	) -> Self {
		let mut imported_names = HashMap::new();
		for import in parsed.get_imports() {
			if let Some(n) = import.name.clone() {
				imported_names.push(n);
				continue;
			}
			// the path in a wildcard import has to be a module
			if import.path.is_empty() {
				panic!("Imported root")
			}
			let abs_path = match absolute_path(&path, &import.path) {
				Ok(p) => p,
				Err(e) => {
					rep.report(e.err_obj(&ctx.i, import.sr.pos(), &path.to_string()).await);
					continue;
				},
			};
			let names = if let Some(subpath) = abs_path.strip_prefix(&pars_root_path[..]) {
				let pars_path = (path.strip_prefix(&pars_root_path[..]))
					.expect("pars path outside pars root");
				resolv_glob(&pars_path, pars_root, &subpath, import.sr.pos(), &ctx.i, rep, &mut ()).await
			} else {
				resolv_glob(&path, root, &abs_path, import.sr.pos(), &ctx.i, rep, &mut ()).await
			}
		}
		todo!()
	}
}
impl Tree for Module {
	type Ctx = HashMap<Sym, Expr>;
	async fn walk<I: IntoIterator<Item = Tok<String>>>(
		&self,
		public_only: bool,
		path: I,
		ctx: &'_ mut Self::Ctx,
	) -> Result<&Self, crate::parsed::WalkError> {
		let mut cur = self;
		for (pos, step) in path.into_iter().enumerate() {
			let Some(member) = self.members.get(&step) else {
				return Err(WalkError{ pos, kind: WalkErrorKind::Missing })
			};
			if public_only && !member.public {
				return Err(WalkError { pos, kind: WalkErrorKind::Private })
			}
			match &member.kind {
				MemberKind::Module(m) => cur = m,
				MemberKind::Alias()
			}
		}
	}
}

pub struct Member {
	pub public: bool,
	pub path: Sym,
	pub lazy: RefCell<Option<LazyMemberHandle>>,
	pub kind: OnceCell<MemberKind>,
}
impl Member {
	pub async fn kind_mut(&mut self, consts: &mut HashMap<Sym, Expr>) -> &mut MemberKind {
		self.kind(consts).await;
		self.kind.get_mut().expect("Thhe above line should have initialized it")
	}
	pub async fn kind(&self, consts: &mut HashMap<Sym, Expr>) -> &MemberKind {
		(self.kind.get_or_init(async {
			let handle = self.lazy.borrow_mut().take().expect("If kind is uninit, lazy must be Some");
			handle.run(consts).await
		}))
		.await
	}
}

pub enum MemberKind {
	Const,
	Module(Module),
	/// This must be pointing at the final value, not a second alias.
	Alias(Sym),
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
	pub async fn run(self, consts: &mut HashMap<Sym, Expr>) -> MemberKind {
		match self.sys.get_tree(self.id).await {
			api::MemberKind::Const(c) => {
				let mut pctx =
					ExprParseCtx { ctx: self.sys.ctx().clone(), exprs: self.sys.ext().exprs().clone() };
				consts.insert(self.path, Expr::from_api(&c, PathSetBuilder::new(), &mut pctx).await);
				MemberKind::Const
			},
			api::MemberKind::Module(m) => MemberKind::Module(
				Module::from_api(m, &mut TreeFromApiCtx { sys: &self.sys, consts, path: self.path.tok() })
					.await,
			),
			api::MemberKind::Lazy(id) => Self { id, ..self }.run(consts).boxed_local().await,
			api::MemberKind::Import(path) => MemberKind::Alias(Sym::from_api(path, self.sys.i()).await),
		}
	}
	pub async fn into_member(self, public: bool, path: Sym) -> Member {
		Member { public, path, kind: OnceCell::new(), lazy: RefCell::new(Some(self)) }
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
