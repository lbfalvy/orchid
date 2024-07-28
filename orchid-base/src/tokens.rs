use orchid_api::tree::{Placeholder, PlaceholderKind};

use crate::interner::{deintern, Tok};

#[derive(Clone, Debug)]
pub struct OwnedPh {
  pub name: Tok<String>,
  pub kind: PlaceholderKind,
}
impl OwnedPh {
  pub fn to_api(&self) -> Placeholder {
    Placeholder { name: self.name.marker(), kind: self.kind.clone() }
  }
  pub fn from_api(ph: Placeholder) -> Self { Self { name: deintern(ph.name), kind: ph.kind } }
}
