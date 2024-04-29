//! Encapsulates the macro runner's scaffolding. Relies on a [ProjectTree]
//! loaded by the [super::loader::Loader]

use std::iter;

use crate::error::{ErrorPosition, ProjectError, ProjectErrorObj, ProjectResult, Reporter};
use crate::location::CodeOrigin;
use crate::parse::parsed;
use crate::pipeline::project::{ItemKind, ProjItem, ProjectTree};
use crate::rule::repository::Repo;
use crate::tree::TreeTransforms;

/// Encapsulates the macro repository and the constant list, and allows querying
/// for macro execution results
pub struct MacroRunner {
  /// Optimized catalog of substitution rules
  pub repo: Repo,
  /// Runtime code containing macro invocations
  pub timeout: Option<usize>,
}
impl MacroRunner {
  /// Initialize a macro runner
  pub fn new(tree: &ProjectTree, timeout: Option<usize>, reporter: &Reporter) -> Self {
    let rules = tree.all_rules();
    let repo = Repo::new(rules, reporter);
    Self { repo, timeout }
  }

  /// Process the macros in an expression.
  pub fn process_expr(&self, expr: parsed::Expr) -> ProjectResult<parsed::Expr> {
    match self.timeout {
      None => Ok((self.repo.pass(&expr)).unwrap_or_else(|| expr.clone())),
      Some(limit) => {
        let (o, leftover_gas) = self.repo.long_step(&expr, limit + 1);
        if 0 < leftover_gas {
          return Ok(o);
        }
        Err(MacroTimeout { location: expr.range.origin(), limit }.pack())
      },
    }
  }

  /// Run all macros in the project.
  pub fn run_macros(&self, tree: ProjectTree, reporter: &Reporter) -> ProjectTree {
    ProjectTree(tree.0.map_data(
      |_, item| match &item.kind {
        ItemKind::Const(c) => match self.process_expr(c.clone()) {
          Ok(expr) => ProjItem { kind: ItemKind::Const(expr) },
          Err(e) => {
            reporter.report(e);
            item
          },
        },
        _ => item,
      },
      |_, x| x,
      |_, x| x,
    ))
  }

  /// Obtain an iterator that steps through the preprocessing of a constant
  /// for debugging macros
  pub fn step(&self, mut expr: parsed::Expr) -> impl Iterator<Item = parsed::Expr> + '_ {
    iter::from_fn(move || {
      expr = self.repo.step(&expr)?;
      Some(expr.clone())
    })
  }
}

/// Error raised when a macro runs too long
#[derive(Debug)]
pub struct MacroTimeout {
  location: CodeOrigin,
  limit: usize,
}
impl ProjectError for MacroTimeout {
  const DESCRIPTION: &'static str = "Macro execution has not halted";

  fn message(&self) -> String {
    let Self { limit, .. } = self;
    format!("Macro processing took more than {limit} steps")
  }

  fn one_position(&self) -> CodeOrigin { self.location.clone() }
}

struct MacroErrors(Vec<ProjectErrorObj>);
impl ProjectError for MacroErrors {
  const DESCRIPTION: &'static str = "Errors occurred during macro execution";
  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> + '_ {
    self.0.iter().enumerate().flat_map(|(i, e)| {
      e.positions().map(move |ep| ErrorPosition {
        origin: ep.origin,
        message: Some(match ep.message {
          Some(msg) => format!("Error #{}: {}; {msg}", i + 1, e.message()),
          None => format!("Error #{}: {}", i + 1, e.message()),
        }),
      })
    })
  }
}
