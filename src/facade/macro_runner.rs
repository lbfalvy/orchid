use std::iter;

use hashbrown::HashMap;

use crate::error::{ProjectError, ProjectResult};
use crate::location::CodeLocation;
use crate::name::Sym;
use crate::parse::parsed;
use crate::pipeline::project::{
  ConstReport, ProjectTree,
};
use crate::rule::repository::Repo;

pub struct MacroRunner {
  /// Optimized catalog of substitution rules
  pub repo: Repo,
  /// Runtime code containing macro invocations
  pub consts: HashMap<Sym, ConstReport>,
}
impl MacroRunner {
  pub fn new(tree: &ProjectTree) -> ProjectResult<Self> {
    let rules = tree.all_rules();
    let repo = Repo::new(rules).map_err(|(rule, e)| e.to_project(&rule))?;
    Ok(Self { repo, consts: tree.all_consts().into_iter().collect() })
  }

  pub fn run_macros(
    &self,
    timeout: Option<usize>,
  ) -> ProjectResult<HashMap<Sym, ConstReport>> {
    let mut symbols = HashMap::new();
    for (name, report) in self.consts.iter() {
      let value = match timeout {
        None => (self.repo.pass(&report.value))
          .unwrap_or_else(|| report.value.clone()),
        Some(limit) => {
          let (o, leftover_gas) = self.repo.long_step(&report.value, limit + 1);
          match leftover_gas {
            1.. => o,
            _ => {
              let err = MacroTimeout {
                location: CodeLocation::Source(report.range.clone()),
                symbol: name.clone(),
                limit,
              };
              return Err(err.pack());
            },
          }
        },
      };
      symbols.insert(name.clone(), ConstReport { value, ..report.clone() });
    }
    Ok(symbols)
  }

  /// Obtain an iterator that steps through the preprocessing of a constant
  /// for debugging macros
  pub fn step(&self, sym: Sym) -> impl Iterator<Item = parsed::Expr> + '_ {
    let mut target =
      self.consts.get(&sym).expect("Target not found").value.clone();
    iter::from_fn(move || {
      target = self.repo.step(&target)?;
      Some(target.clone())
    })
  }
}

/// Error raised when a macro runs too long
#[derive(Debug)]
pub struct MacroTimeout {
  location: CodeLocation,
  symbol: Sym,
  limit: usize,
}
impl ProjectError for MacroTimeout {
  const DESCRIPTION: &'static str = "Macro execution has not halted";

  fn message(&self) -> String {
    let Self { symbol, limit, .. } = self;
    format!("Macro processing in {symbol} took more than {limit} steps")
  }

  fn one_position(&self) -> CodeLocation { self.location.clone() }
}
