use std::num::NonZero;
use std::rc::Rc;

use dyn_clone::{DynClone, clone_box};
use futures::FutureExt;
use futures::future::{LocalBoxFuture, join_all};
use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use orchid_base::interner::{Interner, Tok};
use orchid_base::location::Pos;
use orchid_base::name::Sym;
use orchid_base::reqnot::ReqHandlish;
use orchid_base::tree::{TokTree, Token, TokenVariant};
use substack::Substack;
use trait_set::trait_set;

use crate::api;
use crate::conv::ToExpr;
use crate::entrypoint::MemberRecord;
use crate::expr::{Expr, ExprHandle};
use crate::func_atom::{ExprFunc, Fun};
use crate::gen_expr::{GExpr, arg, call, lambda, seq};
use crate::system::SysCtx;

pub type GenTokTree = TokTree<Expr, GExpr>;
pub type GenTok = Token<Expr, GExpr>;

impl TokenVariant<api::Expression> for GExpr {
	type FromApiCtx<'a> = ();
	type ToApiCtx<'a> = (SysCtx, &'a dyn ReqHandlish);
	async fn from_api(
		_: &api::Expression,
		_: &mut Self::FromApiCtx<'_>,
		_: Pos,
		_: &Interner,
	) -> Self {
		panic!("Received new expression from host")
	}
	async fn into_api(self, (ctx, hand): &mut Self::ToApiCtx<'_>) -> api::Expression {
		self.api_return(ctx.clone(), hand).await
	}
}

impl TokenVariant<api::ExprTicket> for Expr {
	type FromApiCtx<'a> = SysCtx;
	async fn from_api(
		api: &api::ExprTicket,
		ctx: &mut Self::FromApiCtx<'_>,
		_: Pos,
		_: &Interner,
	) -> Self {
		// SAFETY: receiving trees from sublexers implies ownership transfer
		Expr::from_handle(Rc::new(unsafe { ExprHandle::from_args(ctx.clone(), *api) }))
	}
	type ToApiCtx<'a> = ();
	async fn into_api(self, (): &mut Self::ToApiCtx<'_>) -> api::ExprTicket {
		let hand = self.handle();
		std::mem::drop(self);
		let h = match Rc::try_unwrap(hand) {
			Ok(h) => h,
			Err(h) => h.as_ref().clone().await,
		};
		h.into_tk()
	}
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
			GenItemKind::Export(n) => api::ItemKind::Export(ctx.sys().i().i::<String>(&n).await.to_api()),
			GenItemKind::Member(mem) => api::ItemKind::Member(mem.into_api(ctx).await),
			GenItemKind::Import(cn) => api::ItemKind::Import(
				Sym::parse(&cn, ctx.sys().i()).await.expect("Import path empty string").to_api(),
			),
		};
		let comments = join_all(self.comments.iter().map(|c| async {
			api::Comment {
				location: api::Location::Inherit,
				text: ctx.sys().i().i::<String>(c).await.to_api(),
			}
		}))
		.await;
		api::Item { location: api::Location::Inherit, comments, kind }
	}
}

pub fn cnst(public: bool, name: &str, value: impl ToExpr) -> Vec<GenItem> {
	with_export(GenMember { name: name.to_string(), kind: MemKind::Const(value.to_expr()) }, public)
}
pub fn import(public: bool, path: &str) -> Vec<GenItem> {
	let mut out = vec![GenItemKind::Import(path.to_string())];
	if public {
		out.push(GenItemKind::Export(path.split("::").last().unwrap().to_string()));
	}
	out.into_iter().map(|kind| GenItem { comments: vec![], kind }).collect()
}
pub fn module(
	public: bool,
	name: &str,
	items: impl IntoIterator<Item = Vec<GenItem>>,
) -> Vec<GenItem> {
	let (name, kind) = root_mod(name, items);
	with_export(GenMember { name, kind }, public)
}
pub fn root_mod(name: &str, items: impl IntoIterator<Item = Vec<GenItem>>) -> (String, MemKind) {
	let kind = MemKind::Mod { items: items.into_iter().flatten().collect() };
	(name.to_string(), kind)
}
pub fn fun<I, O>(exported: bool, name: &str, xf: impl ExprFunc<I, O>) -> Vec<GenItem> {
	let fac = LazyMemberFactory::new(move |sym, ctx| async {
		return MemKind::Const(build_lambdas(Fun::new(sym, ctx, xf).await, 0));
		fn build_lambdas(fun: Fun, i: u64) -> GExpr {
			if i < fun.arity().into() {
				return lambda(i, [build_lambdas(fun, i + 1)]);
			}
			let arity = fun.arity();
			seq(
				(0..arity)
					.map(|i| arg(i as u64))
					.chain([call([fun.to_expr()].into_iter().chain((0..arity).map(|i| arg(i as u64))))]),
			)
		}
	});
	with_export(GenMember { name: name.to_string(), kind: MemKind::Lazy(fac) }, exported)
}
pub fn prefix(path: &str, items: impl IntoIterator<Item = Vec<GenItem>>) -> Vec<GenItem> {
	let mut items = items.into_iter().flatten().collect_vec();
	for step in path.split("::").collect_vec().into_iter().rev() {
		items = module(true, step, [items]);
	}
	items
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

/// Trivially merge a gen tree. Behaviours were chosen to make this simple.
///
/// - Comments on imports are discarded
/// - Comments on exports and submodules are combined
/// - Duplicate constants result in an error
/// - A combination of lazy and anything results in an error
pub fn merge_trivial(trees: impl IntoIterator<Item = Vec<GenItem>>) -> Vec<GenItem> {
	let mut imported = HashSet::<String>::new();
	let mut exported = HashMap::<String, HashSet<String>>::new();
	let mut members = HashMap::<String, (MemKind, HashSet<String>)>::new();
	for item in trees.into_iter().flatten() {
		match item.kind {
			GenItemKind::Import(sym) => {
				imported.insert(sym);
			},
			GenItemKind::Export(e) =>
				exported.entry(e.clone()).or_insert(HashSet::new()).extend(item.comments.iter().cloned()),
			GenItemKind::Member(mem) => match mem.kind {
				unit @ (MemKind::Const(_) | MemKind::Lazy(_)) => {
					let prev = members.insert(mem.name.clone(), (unit, item.comments.into_iter().collect()));
					assert!(prev.is_none(), "Conflict in trivial tree merge on {}", mem.name);
				},
				MemKind::Mod { items } => match members.entry(mem.name.clone()) {
					hashbrown::hash_map::Entry::Vacant(slot) => {
						slot.insert((MemKind::Mod { items }, item.comments.into_iter().collect()));
					},
					hashbrown::hash_map::Entry::Occupied(mut old) => match old.get_mut() {
						(MemKind::Mod { items: old_items }, old_cmts) => {
							let mut swap = vec![];
							std::mem::swap(&mut swap, old_items);
							*old_items = merge_trivial([swap, items]);
							old_cmts.extend(item.comments);
						},
						_ => panic!("Conflict in trivial merge on {}", mem.name),
					},
				},
			},
		}
	}

	(imported.into_iter().map(|txt| GenItem { comments: vec![], kind: GenItemKind::Import(txt) }))
		.chain(exported.into_iter().map(|(k, cmtv)| GenItem {
			comments: cmtv.into_iter().collect(),
			kind: GenItemKind::Export(k),
		}))
		.chain(members.into_iter().map(|(name, (kind, cmtv))| GenItem {
			comments: cmtv.into_iter().collect(),
			kind: GenItemKind::Member(GenMember { name, kind }),
		}))
		.collect()
}

trait_set! {
	trait LazyMemberCallback =
		FnOnce(Sym, SysCtx) -> LocalBoxFuture<'static, MemKind> + DynClone
}
pub struct LazyMemberFactory(Box<dyn LazyMemberCallback>);
impl LazyMemberFactory {
	pub fn new(cb: impl AsyncFnOnce(Sym, SysCtx) -> MemKind + Clone + 'static) -> Self {
		Self(Box::new(|s, ctx| cb(s, ctx).boxed_local()))
	}
	pub async fn build(self, path: Sym, ctx: SysCtx) -> MemKind { (self.0)(path, ctx).await }
}
impl Clone for LazyMemberFactory {
	fn clone(&self) -> Self { Self(clone_box(&*self.0)) }
}

pub enum GenItemKind {
	Member(GenMember),
	Export(String),
	Import(String),
}

pub struct GenMember {
	pub name: String,
	pub kind: MemKind,
}
impl GenMember {
	pub async fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> api::Member {
		let name = ctx.sys().i().i::<String>(&self.name).await;
		api::Member {
			kind: self.kind.into_api(&mut ctx.push_path(name.clone())).await,
			name: name.to_api(),
		}
	}
}

pub enum MemKind {
	Const(GExpr),
	Mod { items: Vec<GenItem> },
	Lazy(LazyMemberFactory),
}
impl MemKind {
	pub async fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> api::MemberKind {
		match self {
			Self::Lazy(lazy) => api::MemberKind::Lazy(ctx.with_lazy(lazy)),
			Self::Const(c) => api::MemberKind::Const(c.api_return(ctx.sys(), ctx.req()).await),
			Self::Mod { items } => {
				let mut api_items = Vec::new();
				for i in items {
					api_items.push(i.into_api(ctx).boxed_local().await)
				}
				api::MemberKind::Module(api::Module { items: api_items })
			},
		}
	}
}

pub trait TreeIntoApiCtx {
	fn sys(&self) -> SysCtx;
	fn with_lazy(&mut self, fac: LazyMemberFactory) -> api::TreeId;
	fn push_path(&mut self, seg: Tok<String>) -> impl TreeIntoApiCtx;
	fn req(&self) -> &impl ReqHandlish;
}

pub struct TreeIntoApiCtxImpl<'a, 'b, RH: ReqHandlish> {
	pub sys: SysCtx,
	pub basepath: &'a [Tok<String>],
	pub path: Substack<'a, Tok<String>>,
	pub lazy_members: &'b mut HashMap<api::TreeId, MemberRecord>,
	pub req: &'a RH,
}

impl<RH: ReqHandlish> TreeIntoApiCtx for TreeIntoApiCtxImpl<'_, '_, RH> {
	fn sys(&self) -> SysCtx { self.sys.clone() }
	fn push_path(&mut self, seg: Tok<String>) -> impl TreeIntoApiCtx {
		TreeIntoApiCtxImpl {
			req: self.req,
			lazy_members: self.lazy_members,
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
	fn req(&self) -> &impl ReqHandlish { self.req }
}
