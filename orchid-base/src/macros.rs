use itertools::Itertools;
use never::Never;
use trait_set::trait_set;

use crate::{match_mapping, name::Sym, tree::{Paren, Ph}};
use std::{marker::PhantomData, sync::Arc};

use crate::{api, location::Pos};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct MacroSlot<'a>(api::MacroTreeId, PhantomData<&'a ()>);
impl<'a> MacroSlot<'a> {
  pub fn id(self) -> api::MacroTreeId { self.0 }
}

trait_set! {
  pub trait MacroAtomToApi<A> = FnMut(&A) -> api::MacroToken;
  pub trait MacroAtomFromApi<'a, A> = FnMut(&api::Atom) -> MTok<'a, A>;
}

#[derive(Clone, Debug)]
pub struct MTree<'a, A> {
  pub pos: Pos,
  pub tok: Arc<MTok<'a, A>>
}
impl<'a, A> MTree<'a, A> {
  pub(crate) fn from_api(api: &api::MacroTree, do_atom: &mut impl MacroAtomFromApi<'a, A>) -> Self {
    Self { pos: Pos::from_api(&api.location), tok: Arc::new(MTok::from_api(&api.token, do_atom)) }
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
  Ref(Box<MTok<'a, Never>>),
}
impl<'a, A> MTok<'a, A> {
  pub(crate) fn from_api(
    api: &api::MacroToken,
    do_atom: &mut impl MacroAtomFromApi<'a, A>
  ) -> Self {
    match_mapping!(&api, api::MacroToken => MTok::<'a, A> {
      Lambda(x => mtreev_from_api(x, do_atom), b => mtreev_from_api(b, do_atom)),
      Name(t => Sym::from_api(*t)),
      Slot(tk => MacroSlot(*tk, PhantomData)),
      S(p.clone(), b => mtreev_from_api(b, do_atom)),
      Ph(ph => Ph::from_api(ph)),
    } {
      api::MacroToken::Atom(a) => do_atom(a)
    })
  }
  pub(crate) fn to_api(&self, do_atom: &mut impl MacroAtomToApi<A>) -> api::MacroToken {
    match_mapping!(&self, MTok => api::MacroToken {
      Lambda(x => mtreev_to_api(x, do_atom), b => mtreev_to_api(b, do_atom)),
      Name(t.tok().to_api()),
      Ph(ph.to_api()),
      S(p.clone(), b => mtreev_to_api(b, do_atom)),
      Slot(tk.0.clone()),
    } {
      MTok::Ref(r) => r.to_api(&mut |e| match *e {}),
      MTok::Atom(a) => do_atom(a),
    })
  }
}

pub fn mtreev_from_api<'a, 'b, A>(
  api: impl IntoIterator<Item = &'b api::MacroTree>,
  do_atom: &mut impl MacroAtomFromApi<'a, A>
) -> Vec<MTree<'a, A>> {
  api.into_iter().map(|api| MTree::from_api(api, do_atom)).collect_vec()
}

pub fn mtreev_to_api<'a: 'b, 'b, A: 'b>(
  v: impl IntoIterator<Item = &'b MTree<'a, A>>,
  do_atom: &mut impl MacroAtomToApi<A>
) -> Vec<api::MacroTree> {
  v.into_iter().map(|t| t.to_api(do_atom)).collect_vec()
}