use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

use async_stream::stream;
use futures::future::LocalBoxFuture;
use futures::{FutureExt, StreamExt};
use never::Never;
use trait_set::trait_set;

use crate::interner::Interner;
use crate::location::Pos;
use crate::name::Sym;
use crate::tree::{Paren, Ph};
use crate::{api, match_mapping};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct MacroSlot<'a>(api::MacroTreeId, PhantomData<&'a ()>);
impl MacroSlot<'_> {
	pub fn id(self) -> api::MacroTreeId { self.0 }
}

trait_set! {
	pub trait MacroAtomToApi<A> = for<'a> FnMut(&'a A) -> LocalBoxFuture<'a, api::MacroToken>;
	pub trait MacroAtomFromApi<'a, A> =
		for<'b> FnMut(&'b api::Atom) -> LocalBoxFuture<'b, MTok<'a, A>>;
}

#[derive(Clone, Debug)]
pub struct MTree<'a, A> {
	pub pos: Pos,
	pub tok: Rc<MTok<'a, A>>,
}
impl<'a, A> MTree<'a, A> {
	pub(crate) async fn from_api(
		api: &api::MacroTree,
		do_atom: &mut impl MacroAtomFromApi<'a, A>,
		i: &Interner,
	) -> Self {
		Self {
			pos: Pos::from_api(&api.location, i).await,
			tok: Rc::new(MTok::from_api(&api.token, i, do_atom).await),
		}
	}
	pub(crate) async fn to_api(&self, do_atom: &mut impl MacroAtomToApi<A>) -> api::MacroTree {
		api::MacroTree {
			location: self.pos.to_api(),
			token: self.tok.to_api(do_atom).boxed_local().await,
		}
	}
}

#[derive(Clone, Debug)]
pub enum MTok<'a, A> {
	S(Paren, Vec<MTree<'a, A>>),
	Name(Sym),
	Slot(MacroSlot<'a>),
	Lambda(Vec<MTree<'a, A>>, Vec<MTree<'a, A>>),
	Ph(Ph),
	Atom(A),
	/// Used in extensions to directly return input
	Ref(Arc<MTok<'a, Never>>),
	/// Used in the matcher to skip previous macro output which can only go in
	/// vectorial placeholders
	Done(Rc<MTok<'a, A>>),
}
impl<'a, A> MTok<'a, A> {
	pub(crate) async fn from_api(
		api: &api::MacroToken,
		i: &Interner,
		do_atom: &mut impl MacroAtomFromApi<'a, A>,
	) -> Self {
		match_mapping!(&api, api::MacroToken => MTok::<'a, A> {
			Lambda(x => mtreev_from_api(x, i, do_atom).await, b => mtreev_from_api(b, i, do_atom).await),
			Name(t => Sym::from_api(*t, i).await),
			Slot(tk => MacroSlot(*tk, PhantomData)),
			S(p.clone(), b => mtreev_from_api(b, i, do_atom).await),
			Ph(ph => Ph::from_api(ph, i).await),
		} {
			api::MacroToken::Atom(a) => do_atom(a).await
		})
	}
	pub(crate) async fn to_api(&self, do_atom: &mut impl MacroAtomToApi<A>) -> api::MacroToken {
		fn sink<T>(n: &Never) -> LocalBoxFuture<'_, T> { match *n {} }
		match_mapping!(&self, MTok => api::MacroToken {
			Lambda(x => mtreev_to_api(x, do_atom).await, b => mtreev_to_api(b, do_atom).await),
			Name(t.tok().to_api()),
			Ph(ph.to_api()),
			S(p.clone(), b => mtreev_to_api(b, do_atom).await),
			Slot(tk.0.clone()),
		} {
			MTok::Ref(r) => r.to_api(&mut sink).boxed_local().await,
			MTok::Done(t) => t.to_api(do_atom).boxed_local().await,
			MTok::Atom(a) => do_atom(a).await,
		})
	}
	pub fn at(self, pos: Pos) -> MTree<'a, A> { MTree { pos, tok: Rc::new(self) } }
}

pub async fn mtreev_from_api<'a, 'b, A>(
	apiv: impl IntoIterator<Item = &'b api::MacroTree>,
	i: &Interner,
	do_atom: &'b mut (impl MacroAtomFromApi<'a, A> + 'b),
) -> Vec<MTree<'a, A>> {
	stream! {
		for api in apiv {
			yield MTree::from_api(api, do_atom, i).boxed_local().await
		}
	}
	.collect()
	.await
}

pub async fn mtreev_to_api<'a: 'b, 'b, A: 'b>(
	v: impl IntoIterator<Item = &'b MTree<'a, A>>,
	do_atom: &mut impl MacroAtomToApi<A>,
) -> Vec<api::MacroTree> {
	let mut out = Vec::new();
	for t in v {
		out.push(t.to_api(do_atom).await);
	}
	out
}
