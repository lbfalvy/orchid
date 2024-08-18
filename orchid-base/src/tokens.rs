use std::fmt::Display;

pub use api::Paren;

use crate::api;
use crate::interner::{deintern, Tok};

pub const PARENS: &[(char, char, Paren)] =
  &[('(', ')', Paren::Round), ('[', ']', Paren::Square), ('{', '}', Paren::Curly)];

#[derive(Clone, Debug)]
pub struct OwnedPh {
  pub name: Tok<String>,
  pub kind: api::PlaceholderKind,
}
impl OwnedPh {
  pub fn to_api(&self) -> api::Placeholder {
    api::Placeholder { name: self.name.marker(), kind: self.kind.clone() }
  }
  pub fn from_api(ph: api::Placeholder) -> Self { Self { name: deintern(ph.name), kind: ph.kind } }
}

impl Display for OwnedPh {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self.kind {
      api::PlaceholderKind::Name => write!(f, "$_{}", self.name),
      api::PlaceholderKind::Scalar => write!(f, "${}", self.name),
      api::PlaceholderKind::Vector { nz: false, prio: 0 } => write!(f, "..${}", self.name),
      api::PlaceholderKind::Vector { nz: true, prio: 0 } => write!(f, "...${}", self.name),
      api::PlaceholderKind::Vector { nz: false, prio } => write!(f, "..${}:{prio}", self.name),
      api::PlaceholderKind::Vector { nz: true, prio } => write!(f, "...${}:{prio}", self.name),
    }
  }
}
