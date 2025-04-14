use std::any::{Any, TypeId, type_name};
use std::fmt;
use std::future::Future;
use std::num::NonZeroU32;
use std::ops::Deref;
use std::pin::Pin;
use std::rc::Rc;

use ahash::HashMap;
use async_std::io::{Read, Write};
use async_std::stream;
use dyn_clone::{DynClone, clone_box};
use futures::future::LocalBoxFuture;
use futures::{FutureExt, StreamExt};
use orchid_api_derive::Coding;
use orchid_api_traits::{Coding, Decode, Encode, Request, enc_vec};
use orchid_base::clone;
use orchid_base::error::{OrcErr, OrcRes, mk_err};
use orchid_base::format::{FmtCtx, FmtUnit, Format};
use orchid_base::interner::Interner;
use orchid_base::location::Pos;
use orchid_base::name::Sym;
use orchid_base::reqnot::Requester;
use trait_set::trait_set;

use crate::api;
// use crate::error::{ProjectError, ProjectResult};
use crate::expr::{Expr, ExprData, ExprHandle, ExprKind};
use crate::gen_expr::GExpr;
use crate::system::{DynSystemCard, SysCtx, atom_info_for, downcast_atom};

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Coding)]
pub struct AtomTypeId(pub NonZeroU32);

pub trait AtomCard: 'static + Sized {
	type Data: Clone + Coding + Sized;
}

pub trait AtomicVariant {}
pub trait Atomic: 'static + Sized {
	type Variant: AtomicVariant;
	type Data: Clone + Coding + Sized + 'static;
	/// Register handlers for IPC calls. If this atom implements [Supports], you
	/// should register your implementations here. If this atom doesn't
	/// participate in IPC at all, the default implementation is fine
	fn reg_reqs() -> MethodSetBuilder<Self> { MethodSetBuilder::new() }
}
impl<A: Atomic> AtomCard for A {
	type Data = <Self as Atomic>::Data;
}

pub trait AtomicFeatures: Atomic {
	fn factory(self) -> AtomFactory;
	type Info: AtomDynfo;
	fn info() -> Self::Info;
	fn dynfo() -> Box<dyn AtomDynfo>;
}
pub trait ToAtom {
	fn to_atom_factory(self) -> AtomFactory;
}
impl<A: AtomicFeatures> ToAtom for A {
	fn to_atom_factory(self) -> AtomFactory { self.factory() }
}
impl ToAtom for AtomFactory {
	fn to_atom_factory(self) -> AtomFactory { self }
}
pub trait AtomicFeaturesImpl<Variant: AtomicVariant> {
	fn _factory(self) -> AtomFactory;
	type _Info: AtomDynfo;
	fn _info() -> Self::_Info;
}
impl<A: Atomic + AtomicFeaturesImpl<A::Variant>> AtomicFeatures for A {
	fn factory(self) -> AtomFactory { self._factory() }
	type Info = <Self as AtomicFeaturesImpl<A::Variant>>::_Info;
	fn info() -> Self::Info { Self::_info() }
	fn dynfo() -> Box<dyn AtomDynfo> { Box::new(Self::info()) }
}

pub fn get_info<A: AtomCard>(
	sys: &(impl DynSystemCard + ?Sized),
) -> (AtomTypeId, Box<dyn AtomDynfo>) {
	atom_info_for(sys, TypeId::of::<A>()).unwrap_or_else(|| {
		panic!("Atom {} not associated with system {}", type_name::<A>(), sys.name())
	})
}

#[derive(Clone)]
pub struct ForeignAtom {
	pub(crate) expr: Rc<ExprHandle>,
	pub(crate) atom: api::Atom,
	pub(crate) pos: Pos,
}
impl ForeignAtom {
	pub fn pos(&self) -> Pos { self.pos.clone() }
	pub fn ctx(&self) -> SysCtx { self.expr.ctx.clone() }
	pub fn ex(self) -> Expr {
		let (handle, pos) = (self.expr.clone(), self.pos.clone());
		let data = ExprData { pos, kind: ExprKind::Atom(ForeignAtom { ..self }) };
		Expr::new(handle, data)
	}
	pub(crate) fn new(handle: Rc<ExprHandle>, atom: api::Atom, pos: Pos) -> Self {
		ForeignAtom { atom, expr: handle, pos }
	}
	pub async fn request<M: AtomMethod>(&self, m: M) -> Option<M::Response> {
		let rep = (self.ctx().reqnot().request(api::Fwd(
			self.atom.clone(),
			Sym::parse(M::NAME, self.ctx().i()).await.unwrap().tok().to_api(),
			enc_vec(&m).await,
		)))
		.await?;
		Some(M::Response::decode(Pin::new(&mut &rep[..])).await)
	}
}
impl fmt::Display for ForeignAtom {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "Atom::{:?}", self.atom) }
}
impl fmt::Debug for ForeignAtom {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "ForeignAtom({self})") }
}
impl Format for ForeignAtom {
	async fn print<'a>(&'a self, _c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		FmtUnit::from_api(&self.ctx().reqnot().request(api::ExtAtomPrint(self.atom.clone())).await)
	}
}

pub struct NotTypAtom {
	pub pos: Pos,
	pub expr: Expr,
	pub typ: Box<dyn AtomDynfo>,
	pub ctx: SysCtx,
}
impl NotTypAtom {
	pub async fn mk_err(&self) -> OrcErr {
		mk_err(
			self.ctx.i().i("Not the expected type").await,
			format!("This expression is not a {}", self.typ.name()),
			[self.pos.clone().into()],
		)
	}
}

pub trait AtomMethod: Request {
	const NAME: &str;
}
pub trait Supports<M: AtomMethod>: AtomCard {
	fn handle(&self, ctx: SysCtx, req: M) -> impl Future<Output = <M as Request>::Response>;
}

trait_set! {
	trait AtomReqCb<A> = for<'a> Fn(
		&'a A,
		SysCtx,
		Pin<&'a mut dyn Read>,
		Pin<&'a mut dyn Write>,
	) -> LocalBoxFuture<'a, ()>
}

pub struct MethodSetBuilder<A: AtomCard> {
	handlers: Vec<(&'static str, Rc<dyn AtomReqCb<A>>)>,
}
impl<A: AtomCard> MethodSetBuilder<A> {
	pub fn new() -> Self { Self { handlers: vec![] } }

	pub fn handle<M: AtomMethod>(mut self) -> Self
	where A: Supports<M> {
		assert!(!M::NAME.is_empty(), "AtomMethod::NAME cannoot be empty");
		self.handlers.push((
			M::NAME,
			Rc::new(move |a: &A, ctx: SysCtx, req: Pin<&mut dyn Read>, rep: Pin<&mut dyn Write>| {
				async { Supports::<M>::handle(a, ctx, M::decode(req).await).await.encode(rep).await }
					.boxed_local()
			}),
		));
		self
	}

	pub async fn pack(&self, ctx: SysCtx) -> MethodSet<A> {
		MethodSet {
			handlers: stream::from_iter(self.handlers.iter())
				.then(|(k, v)| {
					clone!(ctx; async move {
						(Sym::parse(k, ctx.i()).await.unwrap(), v.clone())
					})
				})
				.collect()
				.await,
		}
	}
}

pub struct MethodSet<A: AtomCard> {
	handlers: HashMap<Sym, Rc<dyn AtomReqCb<A>>>,
}
impl<A: AtomCard> MethodSet<A> {
	pub(crate) async fn dispatch<'a>(
		&'a self,
		atom: &'a A,
		ctx: SysCtx,
		key: Sym,
		req: Pin<&'a mut dyn Read>,
		rep: Pin<&'a mut dyn Write>,
	) -> bool {
		match self.handlers.get(&key) {
			None => false,
			Some(handler) => {
				handler(atom, ctx, req, rep).await;
				true
			},
		}
	}
}

impl<A: AtomCard> Default for MethodSetBuilder<A> {
	fn default() -> Self { Self::new() }
}

#[derive(Clone)]
pub struct TypAtom<A: AtomicFeatures> {
	pub data: ForeignAtom,
	pub value: A::Data,
}
impl<A: AtomicFeatures> TypAtom<A> {
	pub async fn downcast(expr: Rc<ExprHandle>) -> Result<Self, NotTypAtom> {
		match Expr::from_handle(expr).atom().await {
			Err(expr) => Err(NotTypAtom {
				ctx: expr.handle().get_ctx(),
				pos: expr.data().await.pos.clone(),
				expr,
				typ: Box::new(A::info()),
			}),
			Ok(atm) => match downcast_atom::<A>(atm).await {
				Ok(tatom) => Ok(tatom),
				Err(fa) => Err(NotTypAtom {
					pos: fa.pos.clone(),
					ctx: fa.ctx().clone(),
					expr: fa.ex(),
					typ: Box::new(A::info()),
				}),
			},
		}
	}
	pub async fn request<M: AtomMethod>(&self, req: M) -> M::Response
	where A: Supports<M> {
		M::Response::decode(Pin::new(
			&mut &(self.data.ctx().reqnot().request(api::Fwd(
				self.data.atom.clone(),
				Sym::parse(M::NAME, self.data.ctx().i()).await.unwrap().tok().to_api(),
				enc_vec(&req).await,
			)))
			.await
			.unwrap()[..],
		))
		.await
	}
}
impl<A: AtomicFeatures> Deref for TypAtom<A> {
	type Target = A::Data;
	fn deref(&self) -> &Self::Target { &self.value }
}

pub struct AtomCtx<'a>(pub &'a [u8], pub Option<api::AtomId>, pub SysCtx);
impl FmtCtx for AtomCtx<'_> {
	fn i(&self) -> &Interner { self.2.i() }
}

pub trait AtomDynfo: 'static {
	fn tid(&self) -> TypeId;
	fn name(&self) -> &'static str;
	fn decode<'a>(&'a self, ctx: AtomCtx<'a>) -> LocalBoxFuture<'a, Box<dyn Any>>;
	fn call<'a>(&'a self, ctx: AtomCtx<'a>, arg: Expr) -> LocalBoxFuture<'a, GExpr>;
	fn call_ref<'a>(&'a self, ctx: AtomCtx<'a>, arg: Expr) -> LocalBoxFuture<'a, GExpr>;
	fn print<'a>(&'a self, ctx: AtomCtx<'a>) -> LocalBoxFuture<'a, FmtUnit>;
	fn handle_req<'a, 'b: 'a, 'c: 'a>(
		&'a self,
		ctx: AtomCtx<'a>,
		key: Sym,
		req: Pin<&'b mut dyn Read>,
		rep: Pin<&'c mut dyn Write>,
	) -> LocalBoxFuture<'a, bool>;
	fn command<'a>(&'a self, ctx: AtomCtx<'a>) -> LocalBoxFuture<'a, OrcRes<Option<GExpr>>>;
	fn serialize<'a, 'b: 'a>(
		&'a self,
		ctx: AtomCtx<'a>,
		write: Pin<&'b mut dyn Write>,
	) -> LocalBoxFuture<'a, Option<Vec<Expr>>>;
	fn deserialize<'a>(
		&'a self,
		ctx: SysCtx,
		data: &'a [u8],
		refs: &'a [Expr],
	) -> LocalBoxFuture<'a, api::Atom>;
	fn drop<'a>(&'a self, ctx: AtomCtx<'a>) -> LocalBoxFuture<'a, ()>;
}

trait_set! {
	pub trait AtomFactoryFn = FnOnce(SysCtx) -> LocalBoxFuture<'static, api::Atom> + DynClone;
}
pub struct AtomFactory(Box<dyn AtomFactoryFn>);
impl AtomFactory {
	pub fn new(f: impl AsyncFnOnce(SysCtx) -> api::Atom + Clone + 'static) -> Self {
		Self(Box::new(|ctx| f(ctx).boxed_local()))
	}
	pub async fn build(self, ctx: SysCtx) -> api::Atom { (self.0)(ctx).await }
}
impl Clone for AtomFactory {
	fn clone(&self) -> Self { AtomFactory(clone_box(&*self.0)) }
}
impl fmt::Debug for AtomFactory {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "AtomFactory") }
}
impl fmt::Display for AtomFactory {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "AtomFactory") }
}
impl Format for AtomFactory {
	async fn print<'a>(&'a self, _c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		"AtomFactory".to_string().into()
	}
}

pub async fn err_not_callable(i: &Interner) -> OrcErr {
	mk_err(i.i("This atom is not callable").await, "Attempted to apply value as function", [])
}

pub async fn err_not_command(i: &Interner) -> OrcErr {
	mk_err(i.i("This atom is not a command").await, "Settled on an inactionable value", [])
}
