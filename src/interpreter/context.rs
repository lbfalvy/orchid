use hashbrown::HashMap;

use crate::interner::Interner;
use crate::representations::interpreted::ExprInst;
use crate::Sym;

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
impl Return {
  /// Check if gas has run out. Returns false if gas is not being used
  pub fn preempted(&self) -> bool { self.gas.map_or(false, |g| g == 0) }
  /// Returns a general report of the return
  pub fn status(&self) -> ReturnStatus {
    if self.preempted() {
      ReturnStatus::Preempted
    } else if self.inert {
      ReturnStatus::Inert
    } else {
      ReturnStatus::Active
    }
  }
}

/// Possible states of a [Return]
pub enum ReturnStatus {
  /// The data is not normalizable any further
  Inert,
  /// Gas is being used and it ran out
  Preempted,
  /// Normalization stopped for a different reason and should continue.
  Active,
}
