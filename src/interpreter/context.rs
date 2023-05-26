use hashbrown::HashMap;

use crate::interner::{Interner, Sym};
use crate::representations::interpreted::ExprInst;

/// All the data associated with an interpreter run
#[derive(Clone)]
pub struct Context<'a> {
  /// Table used to resolve constants
  pub symbols: &'a HashMap<Sym, ExprInst>,
  /// The interner used for strings internally, so external functions can
  /// deduce referenced constant names on the fly
  pub interner: &'a Interner,
  /// The number of reduction steps the interpreter can take before returning
  pub gas: Option<usize>,
}

/// All the data produced by an interpreter run
#[derive(Clone)]
pub struct Return {
  /// The new expression tree
  pub state: ExprInst,
  /// Leftover [Context::gas] if counted
  pub gas: Option<usize>,
  /// If true, the next run would not modify the expression
  pub inert: bool,
}
