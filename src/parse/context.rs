use std::rc::Rc;

use crate::interner::Interner;
use crate::{Tok, VName};

/// Trait enclosing all context features
///
/// Hiding type parameters in associated types allows for simpler
/// parser definitions
pub trait Context: Clone {
  fn ops(&self) -> &[Tok<String>];
  fn file(&self) -> Rc<VName>;
  fn interner(&self) -> &Interner;
}

/// Struct implementing context
///
/// Hiding type parameters in associated types allows for simpler
/// parser definitions
pub struct ParsingContext<'a> {
  pub ops: &'a [Tok<String>],
  pub interner: &'a Interner,
  pub file: Rc<VName>,
}

impl<'a> ParsingContext<'a> {
  pub fn new(
    ops: &'a [Tok<String>],
    interner: &'a Interner,
    file: Rc<VName>,
  ) -> Self {
    Self { ops, interner, file }
  }
}

impl<'a> Clone for ParsingContext<'a> {
  fn clone(&self) -> Self {
    Self { ops: self.ops, interner: self.interner, file: self.file.clone() }
  }
}

impl Context for ParsingContext<'_> {
  fn interner(&self) -> &Interner {
    self.interner
  }
  fn file(&self) -> Rc<VName> {
    self.file.clone()
  }
  fn ops(&self) -> &[Tok<String>] {
    self.ops
  }
}
