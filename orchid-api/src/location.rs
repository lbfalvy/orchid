use std::ops::Range;

use orchid_api_derive::Coding;

use crate::intern::TStrv;

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding)]
pub enum Location {
  None,
  Gen(CodeGenInfo),
  Range(SourceRange),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding)]
pub struct SourceRange {
  pub path: TStrv,
  pub range: Range<u32>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding)]
pub struct CodeGenInfo {
  pub generator: String,
  pub details: String,
}
