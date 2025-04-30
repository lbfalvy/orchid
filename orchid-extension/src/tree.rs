use std::num::NonZero;
use std::rc::Rc;

use async_stream::stream;
use dyn_clone::{DynClone, clone_box};
use futures::future::{LocalBoxFuture, join_all};
use futures::{FutureExt, StreamExt};
use hashbrown::HashMap;
use itertools::Itertools;
use orchid_base::interner::{Interner, Tok};
use orchid_base::location::SrcRange;
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
		_: SrcRange,
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
		_: SrcRange,
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

pub fn cnst(public: bool, name: &str, value: impl ToExpr) -> Vec<GenMember> {
	vec![GenMember {
		name: name.to_string(),
		kind: MemKind::Const(value.to_expr()),
		comments: vec![],
		public,
	}]
}
pub fn module(
	public: bool,
	name: &str,
	mems: impl IntoIterator<Item = Vec<GenMember>>,
) -> Vec<GenMember> {
	let (name, kind) = root_mod(name, mems);
	vec![GenMember { name, kind, public, comments: vec![] }]
}
pub fn root_mod(name: &str, mems: impl IntoIterator<Item = Vec<GenMember>>) -> (String, MemKind) {
	let kind = MemKind::Mod { members: mems.into_iter().flatten().collect() };
	(name.to_string(), kind)
}
pub fn fun<I, O>(public: bool, name: &str, xf: impl ExprFunc<I, O>) -> Vec<GenMember> {
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
	vec![GenMember { name: name.to_string(), kind: MemKind::Lazy(fac), public, comments: vec![] }]
}
pub fn prefix(path: &str, items: impl IntoIterator<Item = Vec<GenMember>>) -> Vec<GenMember> {
	let mut items = items.into_iter().flatten().collect_vec();
	for step in path.split("::").collect_vec().into_iter().rev() {
		items = module(true, step, [items]);
	}
	items
}

pub fn comments<'a>(
	cmts: impl IntoIterator<Item = &'a str>,
	mut val: Vec<GenMember>,
) -> Vec<GenMember> {
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
pub fn merge_trivial(trees: impl IntoIterator<Item = Vec<GenMember>>) -> Vec<GenMember> {
	let mut all_members = HashMap::<String, (MemKind, Vec<String>)>::new();
	for mem in trees.into_iter().flatten() {
		assert!(mem.public, "Non-trivial merge in {}", mem.name);
		match mem.kind {
			unit @ (MemKind::Const(_) | MemKind::Lazy(_)) => {
				let prev = all_members.insert(mem.name.clone(), (unit, mem.comments.into_iter().collect()));
				assert!(prev.is_none(), "Conflict in trivial tree merge on {}", mem.name);
			},
			MemKind::Mod { members } => match all_members.entry(mem.name.clone()) {
				hashbrown::hash_map::Entry::Vacant(slot) => {
					slot.insert((MemKind::Mod { members }, mem.comments.into_iter().collect()));
				},
				hashbrown::hash_map::Entry::Occupied(mut old) => match old.get_mut() {
					(MemKind::Mod { members: old_items, .. }, old_cmts) => {
						let mut swap = vec![];
						std::mem::swap(&mut swap, old_items);
						*old_items = merge_trivial([swap, members]);
						old_cmts.extend(mem.comments);
					},
					_ => panic!("non-trivial merge on {}", mem.name),
				},
			},
		}
	}

	(all_members.into_iter())
		.map(|(name, (kind, comments))| GenMember { comments, kind, name, public: true })
		.collect_vec()
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

pub struct GenMember {
	pub name: String,
	pub kind: MemKind,
	pub public: bool,
	pub comments: Vec<String>,
}
impl GenMember {
	pub async fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> api::Member {
		let name = ctx.sys().i().i::<String>(&self.name).await;
		let kind = self.kind.into_api(&mut ctx.push_path(name.clone())).await;
		let comments =
			join_all(self.comments.iter().map(|cmt| async { ctx.sys().i().i(cmt).await.to_api() })).await;
		api::Member { kind, name: name.to_api(), comments, exported: self.public }
	}
}

pub enum MemKind {
	Const(GExpr),
	Mod { members: Vec<GenMember> },
	Lazy(LazyMemberFactory),
}
impl MemKind {
	pub async fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> api::MemberKind {
		match self {
			Self::Lazy(lazy) => api::MemberKind::Lazy(ctx.with_lazy(lazy)),
			Self::Const(c) => api::MemberKind::Const(c.api_return(ctx.sys(), ctx.req()).await),
			Self::Mod { members } => api::MemberKind::Module(api::Module {
				members: Box::pin(stream! { for m in members { yield m.into_api(ctx).await } }.collect())
					.await,
			}),
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
