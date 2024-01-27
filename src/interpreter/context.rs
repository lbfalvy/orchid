use hashbrown::HashMap;

use super::nort::Expr;
use crate::name::Sym;

/// All the data associated with an interpreter run
#[derive(Clone)]
pub struct RunContext<'a> {
  /// Table used to resolve constants
  pub symbols: &'a HashMap<Sym, Expr>,
  /// The number of reduction steps the interpreter can take before returning
  pub gas: Option<usize>,
}
impl<'a> RunContext<'a> {
  /// Consume some gas if it is being counted
  pub fn use_gas(&mut self, amount: usize) {
    if let Some(g) = self.gas.as_mut() {
      *g = g.saturating_sub(amount)
    }
  }
  /// Gas is being counted and there is none left
  pub fn no_gas(&self) -> bool { self.gas == Some(0) }
}

/// All the data produced by an interpreter run
#[derive(Clone)]
pub struct Halt {
  /// The new expression tree
  pub state: Expr,
  /// Leftover [Context::gas] if counted
  pub gas: Option<usize>,
  /// If true, the next run would not modify the expression
  pub inert: bool,
}
impl Halt {
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
