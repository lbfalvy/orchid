use std::marker::PhantomData;
use std::sync::Arc;

use async_std::stream;
use async_std::sync::Mutex;
use futures::{FutureExt, StreamExt};
use itertools::Itertools;
use never::Never;
use trait_set::trait_set;

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
	pub trait MacroAtomToApi<A> = FnMut(&A) -> api::MacroToken;
	pub trait MacroAtomFromApi<'a, A> = FnMut(&api::Atom) -> MTok<'a, A>;
}

#[derive(Clone, Debug)]
pub struct MTree<'a, A> {
	pub pos: Pos,
	pub tok: Arc<MTok<'a, A>>,
}
impl<'a, A> MTree<'a, A> {
	pub(crate) async fn from_api(
		api: &api::MacroTree,
		do_atom: &mut impl MacroAtomFromApi<'a, A>,
	) -> Self {
		Self {
			pos: Pos::from_api(&api.location).await,
			tok: Arc::new(MTok::from_api(&api.token, do_atom).await),
		}
	}
	pub(crate) fn to_api(&self, do_atom: &mut impl MacroAtomToApi<A>) -> api::MacroTree {
		api::MacroTree { location: self.pos.to_api(), token: self.tok.to_api(do_atom) }
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
	Done(Arc<MTok<'a, A>>),
}
impl<'a, A> MTok<'a, A> {
	pub(crate) async fn from_api(
		api: &api::MacroToken,
		do_atom: &mut impl MacroAtomFromApi<'a, A>,
	) -> Self {
		match_mapping!(&api, api::MacroToken => MTok::<'a, A> {
			Lambda(x => mtreev_from_api(x, do_atom).await, b => mtreev_from_api(b, do_atom).await),
			Name(t => Sym::from_api(*t).await),
			Slot(tk => MacroSlot(*tk, PhantomData)),
			S(p.clone(), b => mtreev_from_api(b, do_atom).await),
			Ph(ph => Ph::from_api(ph).await),
		} {
			api::MacroToken::Atom(a) => do_atom(a)
		})
	}
	pub(crate) fn to_api(&self, do_atom: &mut impl MacroAtomToApi<A>) -> api::MacroToken {
		fn sink(n: &Never) -> api::MacroToken { match *n {} }
		match_mapping!(&self, MTok => api::MacroToken {
			Lambda(x => mtreev_to_api(x, do_atom), b => mtreev_to_api(b, do_atom)),
			Name(t.tok().to_api()),
			Ph(ph.to_api()),
			S(p.clone(), b => mtreev_to_api(b, do_atom)),
			Slot(tk.0.clone()),
		} {
			MTok::Ref(r) => r.to_api(&mut sink),
			MTok::Done(t) => t.to_api(do_atom),
			MTok::Atom(a) => do_atom(a),
		})
	}
	pub fn at(self, pos: Pos) -> MTree<'a, A> { MTree { pos, tok: Arc::new(self) } }
}

pub async fn mtreev_from_api<'a, 'b, A>(
	api: impl IntoIterator<Item = &'b api::MacroTree>,
	do_atom: &mut impl MacroAtomFromApi<'a, A>,
) -> Vec<MTree<'a, A>> {
	let do_atom_lk = Mutex::new(do_atom);
	stream::from_iter(api)
		.then(|api| async { MTree::from_api(api, &mut *do_atom_lk.lock().await).boxed_local().await })
		.collect()
		.await
}

pub fn mtreev_to_api<'a: 'b, 'b, A: 'b>(
	v: impl IntoIterator<Item = &'b MTree<'a, A>>,
	do_atom: &mut impl MacroAtomToApi<A>,
) -> Vec<api::MacroTree> {
	v.into_iter().map(|t| t.to_api(do_atom)).collect_vec()
}
