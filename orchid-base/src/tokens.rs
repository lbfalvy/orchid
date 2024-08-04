use std::fmt::Display;

use orchid_api::tree::{Paren, Placeholder, PlaceholderKind};

use crate::interner::{deintern, Tok};

pub const PARENS: &[(char, char, Paren)] =
  &[('(', ')', Paren::Round), ('[', ']', Paren::Square), ('{', '}', Paren::Curly)];

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

impl Display for OwnedPh {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self.kind {
      PlaceholderKind::Name => write!(f, "$_{}", self.name),
      PlaceholderKind::Scalar => write!(f, "${}", self.name),
      PlaceholderKind::Vector { nz: false, prio: 0 } => write!(f, "..${}", self.name),
      PlaceholderKind::Vector { nz: true, prio: 0 } => write!(f, "...${}", self.name),
      PlaceholderKind::Vector { nz: false, prio } => write!(f, "..${}:{prio}", self.name),
      PlaceholderKind::Vector { nz: true, prio } => write!(f, "...${}:{prio}", self.name),
    }
  }
}