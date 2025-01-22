use std::future::Future;
use std::num::NonZero;
use std::ops::Range;
use std::rc::Rc;

use dyn_clone::{DynClone, clone_box};
use futures::FutureExt;
use futures::future::{LocalBoxFuture, join_all};
use hashbrown::HashMap;
use itertools::Itertools;
use orchid_base::interner::Tok;
use orchid_base::name::Sym;
use orchid_base::reqnot::ReqHandlish;
use orchid_base::tree::{TokTree, Token};
use ordered_float::NotNan;
use substack::Substack;
use trait_set::trait_set;

use crate::api;
use crate::atom::{AtomFactory, ForeignAtom};
use crate::conv::ToExpr;
use crate::entrypoint::MemberRecord;
use crate::func_atom::{ExprFunc, Fun};
use crate::gen_expr::GExpr;
use crate::macros::Rule;
use crate::system::SysCtx;

pub type GenTokTree<'a> = TokTree<'a, ForeignAtom<'a>, AtomFactory>;
pub type GenTok<'a> = Token<'a, ForeignAtom<'a>, AtomFactory>;

pub async fn do_extra(f: &AtomFactory, r: Range<u32>, ctx: SysCtx) -> api::TokenTree {
	api::TokenTree { range: r, token: api::Token::Atom(f.clone().build(ctx).await) }
}

fn with_export(mem: GenMember, public: bool) -> Vec<GenItem> {
	(public.then(|| GenItemKind::Export(mem.name.clone())).into_iter())
		.chain([GenItemKind::Member(mem)])
		.map(|kind| GenItem { comments: vec![], kind })
		.collect()
}

pub struct GenItem {
	pub kind: GenItemKind,
	pub comments: Vec<String>,
}
impl GenItem {
	pub async fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> api::Item {
		let kind = match self.kind {
			GenItemKind::Export(n) => api::ItemKind::Export(ctx.sys().i.i::<String>(&n).await.to_api()),
			GenItemKind::Member(mem) => api::ItemKind::Member(mem.into_api(ctx).await),
			GenItemKind::Import(cn) => api::ItemKind::Import(cn.tok().to_api()),
			GenItemKind::Macro(priority, gen_rules) => {
				let mut rules = Vec::with_capacity(gen_rules.len());
				for rule in gen_rules {
					rules.push(rule.into_api(ctx).await)
				}
				api::ItemKind::Macro(api::MacroBlock { priority, rules })
			},
		};
		let comments = join_all(self.comments.iter().map(|c| async {
			api::Comment {
				location: api::Location::Inherit,
				text: ctx.sys().i.i::<String>(c).await.to_api(),
			}
		}))
		.await;
		api::Item { location: api::Location::Inherit, comments, kind }
	}
}

pub fn cnst(public: bool, name: &str, value: impl ToExpr) -> Vec<GenItem> {
	with_export(GenMember { name: name.to_string(), kind: MemKind::Const(value.to_expr()) }, public)
}
pub async fn module(
	public: bool,
	name: &str,
	imports: impl IntoIterator<Item = Sym>,
	items: impl IntoIterator<Item = Vec<GenItem>>,
) -> Vec<GenItem> {
	let (name, kind) = root_mod(name, imports, items).await;
	with_export(GenMember { name, kind }, public)
}
pub async fn root_mod(
	name: &str,
	imports: impl IntoIterator<Item = Sym>,
	items: impl IntoIterator<Item = Vec<GenItem>>,
) -> (String, MemKind) {
	let kind = MemKind::Mod {
		imports: imports.into_iter().collect(),
		items: items.into_iter().flatten().collect(),
	};
	(name.to_string(), kind)
}
pub async fn fun<I, O>(exported: bool, name: &str, xf: impl ExprFunc<I, O>) -> Vec<GenItem> {
	let fac =
		LazyMemberFactory::new(move |sym| async { MemKind::Const(Fun::new(sym, xf).await.to_expr()) });
	with_export(GenMember { name: name.to_string(), kind: MemKind::Lazy(fac) }, exported)
}
pub fn macro_block(prio: Option<f64>, rules: impl IntoIterator<Item = Rule>) -> Vec<GenItem> {
	let prio = prio.map(|p| NotNan::new(p).unwrap());
	vec![GenItem {
		kind: GenItemKind::Macro(prio, rules.into_iter().collect_vec()),
		comments: vec![],
	}]
}

pub fn comments<'a>(
	cmts: impl IntoIterator<Item = &'a str>,
	mut val: Vec<GenItem>,
) -> Vec<GenItem> {
	let cmts = cmts.into_iter().map(|c| c.to_string()).collect_vec();
	for v in val.iter_mut() {
		v.comments.extend(cmts.iter().cloned());
	}
	val
}

trait_set! {
	trait LazyMemberCallback =
		FnOnce(Sym) -> LocalBoxFuture<'static, MemKind> + DynClone
}
pub struct LazyMemberFactory(Box<dyn LazyMemberCallback>);
impl LazyMemberFactory {
	pub fn new<F: Future<Output = MemKind> + 'static>(
		cb: impl FnOnce(Sym) -> F + Clone + 'static,
	) -> Self {
		Self(Box::new(|s| cb(s).boxed_local()))
	}
	pub async fn build(self, path: Sym) -> MemKind { (self.0)(path).await }
}
impl Clone for LazyMemberFactory {
	fn clone(&self) -> Self { Self(clone_box(&*self.0)) }
}

pub enum GenItemKind {
	Member(GenMember),
	Export(String),
	Import(Sym),
	Macro(Option<NotNan<f64>>, Vec<Rule>),
}

pub struct GenMember {
	name: String,
	kind: MemKind,
}
impl GenMember {
	pub async fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> api::Member {
		let name = ctx.sys().i.i::<String>(&self.name).await;
		api::Member {
			kind: self.kind.into_api(&mut ctx.push_path(name.clone())).await,
			name: name.to_api(),
		}
	}
}

pub enum MemKind {
	Const(GExpr),
	Mod { imports: Vec<Sym>, items: Vec<GenItem> },
	Lazy(LazyMemberFactory),
}
impl MemKind {
	pub async fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> api::MemberKind {
		match self {
			Self::Lazy(lazy) => api::MemberKind::Lazy(ctx.with_lazy(lazy)),
			Self::Const(c) => api::MemberKind::Const(c.api_return(ctx.sys(), ctx.req()).await),
			Self::Mod { imports, items } => {
				let all_items = (imports.into_iter())
					.map(|t| GenItem { comments: vec![], kind: GenItemKind::Import(t) })
					.chain(items);
				let mut items = Vec::new();
				for i in all_items {
					items.push(i.into_api(ctx).boxed_local().await)
				}
				api::MemberKind::Module(api::Module { items })
			},
		}
	}
}

pub trait TreeIntoApiCtx {
	fn sys(&self) -> SysCtx;
	fn with_lazy(&mut self, fac: LazyMemberFactory) -> api::TreeId;
	fn with_rule(&mut self, rule: Rc<Rule>) -> api::MacroId;
	fn push_path(&mut self, seg: Tok<String>) -> impl TreeIntoApiCtx;
	fn req(&self) -> &impl ReqHandlish;
}

pub struct TIACtxImpl<'a, 'b, RH: ReqHandlish> {
	pub sys: SysCtx,
	pub basepath: &'a [Tok<String>],
	pub path: Substack<'a, Tok<String>>,
	pub lazy_members: &'b mut HashMap<api::TreeId, MemberRecord>,
	pub rules: &'b mut HashMap<api::MacroId, Rc<Rule>>,
	pub req: &'a RH,
}

impl<RH: ReqHandlish> TreeIntoApiCtx for TIACtxImpl<'_, '_, RH> {
	fn sys(&self) -> SysCtx { self.sys.clone() }
	fn push_path(&mut self, seg: Tok<String>) -> impl TreeIntoApiCtx {
		TIACtxImpl {
			req: self.req,
			lazy_members: self.lazy_members,
			rules: self.rules,
			sys: self.sys.clone(),
			basepath: self.basepath,
			path: self.path.push(seg),
		}
	}
	fn with_lazy(&mut self, fac: LazyMemberFactory) -> api::TreeId {
		let id = api::TreeId(NonZero::new((self.lazy_members.len() + 2) as u64).unwrap());
		let path = self.basepath.iter().cloned().chain(self.path.unreverse()).collect_vec();
		self.lazy_members.insert(id, MemberRecord::Gen(path, fac));
		id
	}
	fn with_rule(&mut self, rule: Rc<Rule>) -> orchid_api::MacroId {
		let id = api::MacroId(NonZero::new((self.lazy_members.len() + 1) as u64).unwrap());
		self.rules.insert(id, rule);
		id
	}
	fn req(&self) -> &impl ReqHandlish { self.req }
}
