use std::rc::Rc;

use crate::interner::Interner;

/// Trait enclosing all context features
/// 
/// Hiding type parameters in associated types allows for simpler
/// parser definitions
pub trait Context: Clone {
  type Op: AsRef<str>;

  fn ops<'a>(&'a self) -> &'a [Self::Op];
  fn file(&self) -> Rc<Vec<String>>;
  fn interner<'a>(&'a self) -> &'a Interner;
}

/// Struct implementing context
/// 
/// Hiding type parameters in associated types allows for simpler
/// parser definitions
pub struct ParsingContext<'a, Op> {
  pub ops: &'a [Op],
  pub interner: &'a Interner,
  pub file: Rc<Vec<String>>
}

impl<'a, Op> ParsingContext<'a, Op> {
  pub fn new(ops: &'a [Op], interner: &'a Interner, file: Rc<Vec<String>>)
  -> Self { Self { ops, interner, file } }
}

impl<'a, Op> Clone for ParsingContext<'a, Op> {
  fn clone(&self) -> Self {
    Self {
      ops: self.ops,
      interner: self.interner,
      file: self.file.clone()
    }
  }
}

impl<Op: AsRef<str>> Context for ParsingContext<'_, Op> {
  type Op = Op;

  fn interner<'a>(&'a self) -> &'a Interner { self.interner }
  fn file(&self) -> Rc<Vec<String>> {self.file.clone()}
  fn ops<'a>(&'a self) -> &'a [Self::Op] { self.ops }
}