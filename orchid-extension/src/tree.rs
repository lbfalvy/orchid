use std::num::NonZero;
use std::ops::Range;

use dyn_clone::{DynClone, clone_box};
use hashbrown::HashMap;
use itertools::Itertools;
use orchid_base::interner::{Tok, intern};
use orchid_base::location::Pos;
use orchid_base::name::Sym;
use orchid_base::parse::Comment;
use orchid_base::tree::{TokTree, Token};
use ordered_float::NotNan;
use substack::Substack;
use trait_set::trait_set;

use crate::api;
use crate::atom::{AtomFactory, ForeignAtom};
use crate::conv::ToExpr;
use crate::entrypoint::MemberRecord;
use crate::expr::Expr;
use crate::func_atom::{ExprFunc, Fun};
use crate::macros::Rule;
use crate::system::SysCtx;

pub type GenTokTree<'a> = TokTree<'a, ForeignAtom<'a>, AtomFactory>;
pub type GenTok<'a> = Token<'a, ForeignAtom<'a>, AtomFactory>;

pub fn do_extra(f: &AtomFactory, r: Range<u32>, ctx: SysCtx) -> api::TokenTree {
	api::TokenTree { range: r, token: api::Token::Atom(f.clone().build(ctx)) }
}

fn with_export(mem: GenMember, public: bool) -> Vec<GenItem> {
	(public.then(|| GenItemKind::Export(mem.name.clone()).at(Pos::Inherit)).into_iter())
		.chain([GenItemKind::Member(mem).at(Pos::Inherit)])
		.collect()
}

pub struct GenItem {
	pub kind: GenItemKind,
	pub comments: Vec<Comment>,
	pub pos: Pos,
}
impl GenItem {
	pub fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> api::Item {
		let kind = match self.kind {
			GenItemKind::Export(n) => api::ItemKind::Export(n.to_api()),
			GenItemKind::Member(mem) => api::ItemKind::Member(mem.into_api(ctx)),
			GenItemKind::Import(cn) => api::ItemKind::Import(cn.tok().to_api()),
			GenItemKind::Macro(prio, rules) => api::ItemKind::Macro(api::MacroBlock {
				priority: prio,
				rules: rules.into_iter().map(|r| r.to_api()).collect_vec(),
			}),
		};
		let comments = self.comments.into_iter().map(|c| c.to_api()).collect_vec();
		api::Item { location: self.pos.to_api(), comments, kind }
	}
}

pub fn cnst(public: bool, name: &str, value: impl ToExpr) -> Vec<GenItem> {
	with_export(GenMember { name: intern(name), kind: MemKind::Const(value.to_expr()) }, public)
}
pub fn module(
	public: bool,
	name: &str,
	imports: impl IntoIterator<Item = Sym>,
	items: impl IntoIterator<Item = Vec<GenItem>>,
) -> Vec<GenItem> {
	let (name, kind) = root_mod(name, imports, items);
	with_export(GenMember { name, kind }, public)
}
pub fn root_mod(
	name: &str,
	imports: impl IntoIterator<Item = Sym>,
	items: impl IntoIterator<Item = Vec<GenItem>>,
) -> (Tok<String>, MemKind) {
	let kind = MemKind::Mod {
		imports: imports.into_iter().collect(),
		items: items.into_iter().flatten().collect(),
	};
	(intern(name), kind)
}
pub fn fun<I, O>(exported: bool, name: &str, xf: impl ExprFunc<I, O>) -> Vec<GenItem> {
	let fac = LazyMemberFactory::new(move |sym| MemKind::Const(Fun::new(sym, xf).to_expr()));
	with_export(GenMember { name: intern(name), kind: MemKind::Lazy(fac) }, exported)
}
pub fn macro_block(prio: Option<f64>, rules: impl IntoIterator<Item = Rule>) -> Vec<GenItem> {
	let prio = prio.map(|p| NotNan::new(p).unwrap());
	vec![GenItemKind::Macro(prio, rules.into_iter().collect_vec()).gen()]
}

pub fn comments<'a>(
	cmts: impl IntoIterator<Item = &'a str> + Clone,
	mut val: Vec<GenItem>,
) -> Vec<GenItem> {
	for v in val.iter_mut() {
		v.comments
			.extend(cmts.clone().into_iter().map(|c| Comment { text: intern(c), pos: Pos::Inherit }));
	}
	val
}

trait_set! {
	trait LazyMemberCallback = FnOnce(Sym) -> MemKind + Send + Sync + DynClone
}
pub struct LazyMemberFactory(Box<dyn LazyMemberCallback>);
impl LazyMemberFactory {
	pub fn new(cb: impl FnOnce(Sym) -> MemKind + Send + Sync + Clone + 'static) -> Self {
		Self(Box::new(cb))
	}
	pub fn build(self, path: Sym) -> MemKind { (self.0)(path) }
}
impl Clone for LazyMemberFactory {
	fn clone(&self) -> Self { Self(clone_box(&*self.0)) }
}

pub enum GenItemKind {
	Member(GenMember),
	Export(Tok<String>),
	Import(Sym),
	Macro(Option<NotNan<f64>>, Vec<Rule>),
}
impl GenItemKind {
	pub fn at(self, pos: Pos) -> GenItem { GenItem { kind: self, comments: vec![], pos } }
	pub fn gen(self) -> GenItem { GenItem { kind: self, comments: vec![], pos: Pos::Inherit } }
	pub fn gen_equiv(self, comments: Vec<Comment>) -> GenItem {
		GenItem { kind: self, comments, pos: Pos::Inherit }
	}
}

pub struct GenMember {
	name: Tok<String>,
	kind: MemKind,
}
impl GenMember {
	pub fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> api::Member {
		api::Member {
			name: self.name.to_api(),
			kind: self.kind.into_api(&mut ctx.push_path(self.name)),
		}
	}
}

pub enum MemKind {
	Const(Expr),
	Mod { imports: Vec<Sym>, items: Vec<GenItem> },
	Lazy(LazyMemberFactory),
}
impl MemKind {
	pub fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> api::MemberKind {
		match self {
			Self::Lazy(lazy) => api::MemberKind::Lazy(ctx.with_lazy(lazy)),
			Self::Const(c) =>
				api::MemberKind::Const(c.api_return(ctx.sys(), &mut |_| panic!("Slot in const tree"))),
			Self::Mod { imports, items } => api::MemberKind::Module(api::Module {
				items: (imports.into_iter())
					.map(|t| GenItemKind::Import(t).gen())
					.chain(items)
					.map(|i| i.into_api(ctx))
					.collect_vec(),
			}),
		}
	}
}

pub trait TreeIntoApiCtx {
	fn sys(&self) -> SysCtx;
	fn with_lazy(&mut self, fac: LazyMemberFactory) -> api::TreeId;
	fn push_path(&mut self, seg: Tok<String>) -> impl TreeIntoApiCtx;
}

pub struct TIACtxImpl<'a, 'b> {
	pub sys: SysCtx,
	pub basepath: &'a [Tok<String>],
	pub path: Substack<'a, Tok<String>>,
	pub lazy: &'b mut HashMap<api::TreeId, MemberRecord>,
}

impl<'a, 'b> TreeIntoApiCtx for TIACtxImpl<'a, 'b> {
	fn sys(&self) -> SysCtx { self.sys.clone() }
	fn push_path(&mut self, seg: Tok<String>) -> impl TreeIntoApiCtx {
		TIACtxImpl {
			sys: self.sys.clone(),
			lazy: self.lazy,
			basepath: self.basepath,
			path: self.path.push(seg),
		}
	}
	fn with_lazy(&mut self, fac: LazyMemberFactory) -> api::TreeId {
		let id = api::TreeId(NonZero::new((self.lazy.len() + 2) as u64).unwrap());
		let path = Sym::new(self.basepath.iter().cloned().chain(self.path.unreverse())).unwrap();
		self.lazy.insert(id, MemberRecord::Gen(path, fac));
		id
	}
}
