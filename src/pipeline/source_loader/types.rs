use std::fmt::Display;
use std::ops::Add;

use crate::ast::Expr;
use crate::error::ProjectResult;
use crate::sourcefile::Import;
use crate::tree::Module;
use crate::{Interner, Location, VName};

#[derive(Debug, Clone)]
pub struct PreItem {
  pub is_op: bool,
  pub has_value: bool,
  pub location: Location,
}

impl Display for PreItem {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let Self { has_value, is_op, location } = self;
    let description = match (is_op, has_value) {
      (true, true) => "operator with value",
      (true, false) => "operator",
      (false, true) => "value",
      (false, false) => "keyword",
    };
    write!(f, "{description} {location}")
  }
}

impl Default for PreItem {
  fn default() -> Self {
    PreItem { is_op: false, has_value: false, location: Location::Unknown }
  }
}

#[derive(Debug, Clone)]
pub struct PreSubExt {
  pub imports: Vec<Import>,
  pub patterns: Vec<Vec<Expr<VName>>>,
}

#[derive(Debug, Clone)]
pub struct PreFileExt {
  pub name: VName,
  pub details: PreSubExt,
}

#[derive(Debug, Clone)]
pub enum PreExtra {
  File(PreFileExt),
  Submod(PreSubExt),
  Dir,
}

impl PreExtra {
  /// If the module is not a directory, returns the source-only details
  pub fn details(&self) -> Option<&PreSubExt> {
    match self {
      Self::Submod(sub) => Some(sub),
      Self::File(PreFileExt { details, .. }) => Some(details),
      Self::Dir => None,
    }
  }
}

impl Add for PreExtra {
  type Output = ProjectResult<Self>;

  fn add(self, rhs: Self) -> Self::Output {
    match (self, rhs) {
      (alt, Self::Dir) | (Self::Dir, alt) => Ok(alt),
      (Self::File(_) | Self::Submod(_), Self::File(_) | Self::Submod(_)) => {
        panic!("Each file should be parsed once.")
      },
    }
  }
}

impl Display for PreExtra {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Dir => write!(f, "Directory"),
      Self::File(PreFileExt { name, .. }) => {
        write!(f, "File({}.orc)", Interner::extern_all(name).join("/"))
      },
      Self::Submod(_) => write!(f, "Submodule"),
    }
  }
}

pub type PreMod = Module<PreItem, PreExtra>;

#[derive(Debug, Clone)]
pub struct Preparsed(pub PreMod);
