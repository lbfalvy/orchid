use std::iter;
use std::rc::Rc;

use hashbrown::HashMap;

use super::{Process, System};
use crate::error::{ErrorPosition, ProjectError, ProjectResult};
use crate::interpreter::HandlerTable;
use crate::rule::Repo;
use crate::utils::iter::box_once;
use crate::utils::BoxedIter;
use crate::{
  ast, ast_to_interpreted, collect_consts, collect_rules, rule, Interner,
  Location, ProjectTree, Sym,
};

/// Everything needed for macro execution, and constructing the process
pub struct PreMacro<'a> {
  /// Optimized catalog of substitution rules
  pub repo: Repo,
  /// Runtime code containing macro invocations
  pub consts: HashMap<Sym, (ast::Expr<Sym>, Location)>,
  /// Libraries and plug-ins
  pub systems: Vec<System<'a>>,
  /// [Interner] pseudo-global
  pub i: &'a Interner,
}
impl<'a> PreMacro<'a> {
  /// Build a [PreMacro] from a source tree and system list
  pub fn new(
    tree: ProjectTree<Sym>,
    systems: Vec<System<'a>>,
    i: &'a Interner,
  ) -> ProjectResult<Self> {
    let consts = collect_consts(&tree, i);
    let rules = collect_rules(&tree);
    let repo = match rule::Repo::new(rules, i) {
      Ok(r) => r,
      Err((rule, error)) => {
        return Err(error.to_project_error(&rule));
      },
    };
    Ok(Self {
      repo,
      consts: (consts.into_iter())
        .map(|(name, expr)| {
          let location = (i.r(name).split_last())
            .and_then(|(_, path)| {
              let origin = (tree.0.walk_ref(path, false))
                .expect("path sourced from symbol names");
              origin.extra.file.as_ref().map(|path| i.extern_all(&path[..]))
            })
            .map(|p| Location::File(Rc::new(p)))
            .unwrap_or(Location::Unknown);
          (name, (expr, location))
        })
        .collect(),
      i,
      systems,
    })
  }

  /// Run all macros to termination or the optional timeout. If a timeout does
  /// not occur, returns a process which can execute Orchid code
  pub fn build_process(
    self,
    timeout: Option<usize>,
  ) -> ProjectResult<Process<'a>> {
    let Self { i, systems, repo, consts } = self;
    let mut symbols = HashMap::new();
    for (name, (source, source_location)) in consts.iter() {
      let unmatched = if let Some(limit) = timeout {
        let (unmatched, steps_left) = repo.long_step(source, limit + 1);
        if steps_left == 0 {
          return Err(
            MacroTimeout {
              location: source_location.clone(),
              symbol: *name,
              limit,
            }
            .rc(),
          );
        } else {
          unmatched
        }
      } else {
        repo.pass(source).unwrap_or_else(|| source.clone())
      };
      let runtree = ast_to_interpreted(&unmatched).map_err(|e| e.rc())?;
      symbols.insert(*name, runtree);
    }
    Ok(Process {
      symbols,
      i,
      handlers: (systems.into_iter())
        .fold(HandlerTable::new(), |tbl, sys| tbl.combine(sys.handlers)),
    })
  }

  /// Obtain an iterator that steps through the preprocessing of a constant
  /// for debugging macros
  pub fn step(&self, sym: Sym) -> impl Iterator<Item = ast::Expr<Sym>> + '_ {
    let mut target = self.consts.get(&sym).expect("Target not found").0.clone();
    iter::from_fn(move || {
      target = self.repo.step(&target)?;
      Some(target.clone())
    })
  }
}

/// Error raised when a macro runs too long
#[derive(Debug)]
pub struct MacroTimeout {
  location: Location,
  symbol: Sym,
  limit: usize,
}
impl ProjectError for MacroTimeout {
  fn description(&self) -> &str {
    "Macro execution has not halted"
  }

  fn message(&self, i: &Interner) -> String {
    format!(
      "Macro execution during the processing of {} took more than {} steps",
      i.extern_vec(self.symbol).join("::"),
      self.limit
    )
  }

  fn positions(&self, _i: &Interner) -> BoxedIter<ErrorPosition> {
    box_once(ErrorPosition { location: self.location.clone(), message: None })
  }
}
