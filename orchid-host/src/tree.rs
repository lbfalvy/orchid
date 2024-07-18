use std::ops::Range;

use orchid_api::tree::Paren;
use orchid_base::error::OwnedError;
use orchid_base::name::VName;
use orchid_base::tokens::OwnedPh;

use crate::extension::AtomHand;

#[derive(Clone)]
pub struct OwnedTokTree {
  pub tok: OwnedTok,
  pub range: Range<u32>,
}

#[derive(Clone)]
pub enum OwnedTok {
  Lambda(Vec<OwnedTokTree>, Vec<OwnedTokTree>),
  Name(VName),
  S(Paren, Vec<OwnedTokTree>),
  Atom(AtomHand),
  Ph(OwnedPh),
  Bottom(OwnedError),
}
