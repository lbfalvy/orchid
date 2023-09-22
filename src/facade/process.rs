use hashbrown::HashMap;
use itertools::Itertools;

use crate::error::{ErrorPosition, ProjectError, ProjectResult};
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
  #[must_use]
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

  /// Assert that the code contains no invalid constants. This ensures that,
  /// unless [interpreted::Clause::Constant]s are created procedurally,
  /// a [interpreter::RuntimeError::MissingSymbol] cannot be produced
  pub fn validate_refs(&self) -> ProjectResult<()> {
    let mut errors = Vec::new();
    for key in self.symbols.keys() {
      errors.extend(self.unbound_refs(key.clone()).into_iter().map(
        |(symbol, location)| MissingSymbol {
          symbol,
          location,
          referrer: key.clone(),
        },
      ));
    }
    match errors.is_empty() {
      true => Ok(()),
      false => Err(MissingSymbols { errors }.rc()),
    }
  }
}

#[derive(Debug, Clone)]
pub struct MissingSymbol {
  referrer: Sym,
  location: Location,
  symbol: Sym,
}
#[derive(Debug)]
pub struct MissingSymbols {
  errors: Vec<MissingSymbol>,
}
impl ProjectError for MissingSymbols {
  fn description(&self) -> &str {
    "A name not referring to a known symbol was found in the source after \
     macro execution. This can either mean that a symbol name was mistyped, or \
     that macro execution didn't correctly halt."
  }

  fn message(&self) -> String {
    format!(
      "The following symbols do not exist:\n{}",
      (self.errors.iter())
        .map(|e| format!(
          "{} referenced in {} ",
          e.symbol.extern_vec().join("::"),
          e.referrer.extern_vec().join("::")
        ))
        .join("\n")
    )
  }

  fn positions(&self) -> crate::utils::BoxedIter<crate::error::ErrorPosition> {
    Box::new(
      (self.errors.clone().into_iter())
        .map(|i| ErrorPosition { location: i.location, message: None }),
    )
  }
}
