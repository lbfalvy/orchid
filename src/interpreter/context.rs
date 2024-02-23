//! Addiitional information passed to the interpreter

use std::cell::RefCell;
use std::fmt;

use hashbrown::HashMap;

use super::handler::HandlerTable;
use super::nort::{Clause, Expr};
use crate::foreign::error::{RTError, RTErrorObj, RTResult};
use crate::location::CodeLocation;
use crate::name::Sym;

/// Data that must not change except in well-defined ways while any data
/// associated with this process persists
pub struct RunEnv<'a> {
  /// Mutable callbacks the code can invoke with continuation passing
  pub handlers: HandlerTable<'a>,
  /// Constants referenced in the code in [super::nort::Clause::Constant] nodes
  pub symbols: RefCell<HashMap<Sym, RTResult<Expr>>>,
  /// Callback to invoke when a symbol is not found
  pub symbol_cb: Box<dyn Fn(Sym, CodeLocation) -> RTResult<Expr> + 'a>,
}

impl<'a> RunEnv<'a> {
  /// Create a new context. The return values of the symbol callback are cached
  pub fn new(
    handlers: HandlerTable<'a>,
    symbol_cb: impl Fn(Sym, CodeLocation) -> RTResult<Expr> + 'a,
  ) -> Self {
    Self { handlers, symbols: RefCell::new(HashMap::new()), symbol_cb: Box::new(symbol_cb) }
  }

  /// Produce an error indicating that a symbol was missing
  pub fn sym_not_found(sym: Sym, location: CodeLocation) -> RTErrorObj {
    MissingSymbol { location, sym }.pack()
  }

  /// Load a symbol from cache or invoke the callback
  pub fn load(&self, sym: Sym, location: CodeLocation) -> RTResult<Expr> {
    let mut guard = self.symbols.borrow_mut();
    let (_, r) = (guard.raw_entry_mut().from_key(&sym))
      .or_insert_with(|| (sym.clone(), (self.symbol_cb)(sym, location)));
    r.clone()
  }

  /// Attempt to resolve the command with the command handler table
  pub fn dispatch(&self, expr: &Clause, location: CodeLocation) -> Option<Expr> {
    match expr {
      Clause::Atom(at) => self.handlers.dispatch(&*at.0, location),
      _ => None,
    }
  }
}

/// Limits and other context that is subject to change
pub struct RunParams {
  /// Number of reduction steps permitted before the program is preempted
  pub gas: Option<usize>,
  /// Maximum recursion depth. Orchid uses a soft stack so this can be very
  /// large, but it must not be
  pub stack: usize,
}
impl RunParams {
  /// Consume some gas if it is being counted
  pub fn use_gas(&mut self, amount: usize) {
    if let Some(g) = self.gas.as_mut() {
      *g = g.saturating_sub(amount)
    }
  }
  /// Gas is being counted and there is none left
  pub fn no_gas(&self) -> bool { self.gas == Some(0) }
  /// Add gas to make execution longer, or to resume execution in a preempted
  /// expression
  pub fn add_gas(&mut self, amount: usize) {
    if let Some(g) = self.gas.as_mut() {
      *g = g.saturating_add(amount)
    }
  }
}

/// The interpreter's sole output excluding error conditions is an expression
pub type Halt = Expr;

#[derive(Clone)]
pub(crate) struct MissingSymbol {
  pub sym: Sym,
  pub location: CodeLocation,
}
impl RTError for MissingSymbol {}
impl fmt::Display for MissingSymbol {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}, called at {} is not loaded", self.sym, self.location)
  }
}
