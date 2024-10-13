use itertools::Itertools;

use crate::{name::Sym, tree::{AtomTok, Paren, Ph, TokTree}};
use std::marker::PhantomData;

use crate::{api, location::Pos};

#[derive(Clone, Debug)]
pub struct MacroSlot<'a>(api::MacroTreeId, PhantomData<&'a ()>);

#[derive(Clone, Debug)]
pub struct MTree<'a, A: AtomTok> {
  pub pos: Pos,
  pub tok: MTok<'a, A>
}

#[derive(Clone, Debug)]
pub enum MTok<'a, A> {
  S(Paren, Vec<MTree<'a, A>>),
  Name(Sym),
  Slot(MacroSlot<'a>),
  Lambda(Vec<MTree<'a, A>>, Vec<MTree<'a, A>>),
  Ph(Ph),
  Atom(A)
}
impl<'a, A> MTree<'a, A> {
  pub(crate) fn from_api(api: &api::MacroTree) -> Self {
    use api::MacroToken as MTK;
    let tok = match &api.token {
      MTK::Lambda(x, b) => MTok::Lambda(mtreev_from_api(x), mtreev_from_api(b)),
      MTK::Name(t) => MTok::Name(Sym::deintern(*t)),
      MTK::Slot(tk) => MTok::Slot(MacroSlot(tk.clone(), PhantomData)),
      MTK::S(p, b) => MTok::S(p.clone(), mtreev_from_api(b)),
      MTK::Ph(ph) => MTok::Ph(Ph::from_api(ph)),
    };
    Self { pos: Pos::from_api(&api.location), tok }
  }
  pub(crate) fn to_api(&self) -> api::MacroTree {
    use api::MacroToken as MTK;
    let token = match &self.tok {
      MTok::Lambda(x, b) => MTK::Lambda(mtreev_to_api(x), mtreev_to_api(b)),
      MTok::Name(t) => MTK::Name(t.tok().marker()),
      MTok::Ph(ph) => MTK::Ph(ph.to_api()),
      MTok::S(p, b) => MTK::S(p.clone(), mtreev_to_api(b)),
      MTok::Slot(tk) => MTK::Slot(tk.0.clone()),
    };
    api::MacroTree { location: self.pos.to_api(), token }
  }
}

pub fn mtreev_from_api<'a, 'b, A>(
  api: impl IntoIterator<Item = &'b api::MacroTree>
) -> Vec<MTree<'a, A>> {
  api.into_iter().map(MTree::from_api).collect_vec()
}

pub fn mtreev_to_api<'a: 'b, 'b, A: 'b>(
  v: impl IntoIterator<Item = &'b MTree<'a, A>>
) -> Vec<api::MacroTree> {
  v.into_iter().map(MTree::to_api).collect_vec()
}