use std::ops::Range;

use orchid_api_derive::Coding;

use crate::intern::{TStr, TStrv};

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding)]
pub enum Location {
  None,
  /// Used in functions to denote the generated code that carries on the
  /// location of the call. Not allowed in the const tree.
  Inherit,
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
  pub generator: TStr,
  pub details: TStr,
}
