use hashbrown::HashMap;
use itertools::Itertools;

use super::merge_trees::NortConst;
use crate::error::{ErrorPosition, ProjectError, ProjectResult};
use crate::interpreter::context::{Halt, RunContext};
use crate::interpreter::error::RunError;
use crate::interpreter::handler::{run_handler, HandlerTable};
use crate::interpreter::nort::{Clause, Expr};
use crate::location::CodeLocation;
use crate::name::Sym;

/// This struct ties the state of systems to loaded code, and allows to call
/// Orchid-defined functions
pub struct Process<'a> {
  pub(crate) symbols: HashMap<Sym, Expr>,
  pub(crate) handlers: HandlerTable<'a>,
}
impl<'a> Process<'a> {
  /// Build a process from the return value of [crate::facade::merge_trees] and
  pub fn new(
    consts: impl IntoIterator<Item = (Sym, NortConst)>,
    handlers: HandlerTable<'a>,
  ) -> Self {
    let symbols = consts.into_iter().map(|(k, v)| (k, v.value)).collect();
    Self { handlers, symbols }
  }

  /// Execute the given command in this process. If gas is specified, at most as
  /// many steps will be executed and then the partial result returned.
  ///
  /// This is useful to catch infinite loops or ensure that a tenant program
  /// yields
  pub fn run(
    &mut self,
    prompt: Expr,
    gas: Option<usize>,
  ) -> Result<Halt, RunError> {
    let ctx = RunContext { gas, symbols: &self.symbols, stack_size: 1000 };
    run_handler(prompt, &mut self.handlers, ctx)
  }

  /// Find all unbound constant names in a symbol. This is often useful to
  /// identify dynamic loading targets.
  #[must_use]
  pub fn unbound_refs(&self, key: Sym) -> Vec<(Sym, CodeLocation)> {
    let mut errors = Vec::new();
    let sym = self.symbols.get(&key).expect("symbol must exist");
    sym.search_all(&mut |s: &Expr| {
      if let Clause::Constant(sym) = &*s.cls() {
        if !self.symbols.contains_key(sym) {
          errors.push((sym.clone(), s.location()))
        }
      }
      None::<()>
    });
    errors
  }

  /// Assert that the code contains no invalid constants. This ensures that,
  /// unless [Clause::Constant]s are created procedurally,
  /// a [crate::interpreter::error::RunError::MissingSymbol] cannot be produced
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
      false => Err(MissingSymbols { errors }.pack()),
    }
  }
}

#[derive(Debug, Clone)]
struct MissingSymbol {
  referrer: Sym,
  location: CodeLocation,
  symbol: Sym,
}
#[derive(Debug)]
struct MissingSymbols {
  errors: Vec<MissingSymbol>,
}
impl ProjectError for MissingSymbols {
  const DESCRIPTION: &'static str = "A name not referring to a known symbol was found in the source after \
     macro execution. This can either mean that a symbol name was mistyped, or \
     that macro execution didn't correctly halt.";

  fn message(&self) -> String {
    format!(
      "The following symbols do not exist:\n{}",
      (self.errors.iter())
        .map(|MissingSymbol { symbol, referrer, .. }| format!(
          "{symbol} referenced in {referrer}"
        ))
        .join("\n")
    )
  }

  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> {
    (self.errors.iter())
      .map(|i| ErrorPosition { location: i.location.clone(), message: None })
  }
}
