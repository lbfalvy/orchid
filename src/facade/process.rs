use hashbrown::HashMap;

use crate::error::{ProjectError, ProjectResult};
use crate::interpreted::{self, ExprInst};
#[allow(unused)] // for doc
use crate::interpreter;
use crate::interpreter::{
  run_handler, Context, HandlerTable, Return, RuntimeError,
};
use crate::{Interner, Location, Sym};

/// This struct ties the state of systems to loaded code, and allows to call
/// Orchid-defined functions
pub struct Process<'a> {
  pub(crate) symbols: HashMap<Sym, ExprInst>,
  pub(crate) handlers: HandlerTable<'a>,
  pub(crate) i: &'a Interner,
}
impl<'a> Process<'a> {
  /// Execute the given command in this process. If gas is specified, at most as
  /// many steps will be executed and then the partial result returned.
  ///
  /// This is useful to catch infinite loops or ensure that a tenant program
  /// yields
  pub fn run(
    &mut self,
    prompt: ExprInst,
    gas: Option<usize>,
  ) -> Result<Return, RuntimeError> {
    let ctx = Context { gas, interner: self.i, symbols: &self.symbols };
    run_handler(prompt, &mut self.handlers, ctx)
  }

  /// Find all unbound constant names in a symbol. This is often useful to
  /// identify dynamic loading targets.
  pub fn unbound_refs(&self, key: Sym) -> Vec<(Sym, Location)> {
    let mut errors = Vec::new();
    let sym = self.symbols.get(&key).expect("symbol must exist");
    sym.search_all(&mut |s: &ExprInst| {
      let expr = s.expr();
      if let interpreted::Clause::Constant(sym) = &expr.clause {
        if !self.symbols.contains_key(sym) {
          errors.push((sym.clone(), expr.location.clone()))
        }
      }
      None::<()>
    });
    errors
  }

  /// Assert that, unless [interpreted::Clause::Constant]s are created
  /// procedurally, a [interpreter::RuntimeError::MissingSymbol] cannot be
  /// produced
  pub fn validate_refs(&self) -> ProjectResult<()> {
    for key in self.symbols.keys() {
      if let Some((symbol, location)) = self.unbound_refs(key.clone()).pop() {
        return Err(
          MissingSymbol { location, referrer: key.clone(), symbol }.rc(),
        );
      }
    }
    Ok(())
  }
}

#[derive(Debug)]
pub struct MissingSymbol {
  referrer: Sym,
  location: Location,
  symbol: Sym,
}
impl ProjectError for MissingSymbol {
  fn description(&self) -> &str {
    "A name not referring to a known symbol was found in the source after \
     macro execution. This can either mean that a symbol name was mistyped, or \
     that macro execution didn't correctly halt."
  }

  fn message(&self) -> String {
    format!(
      "The symbol {} referenced in {} does not exist",
      self.symbol.extern_vec().join("::"),
      self.referrer.extern_vec().join("::")
    )
  }

  fn one_position(&self) -> Location {
    self.location.clone()
  }
}
