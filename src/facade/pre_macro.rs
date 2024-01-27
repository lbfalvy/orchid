use std::iter;

use hashbrown::HashMap;
use never::Never;

use super::process::Process;
use super::system::System;
use crate::error::{ErrorPosition, ProjectError, ProjectResult};
use crate::intermediate::ast_to_ir::ast_to_ir;
use crate::intermediate::ir_to_nort::ir_to_nort;
use crate::interpreter::handler::HandlerTable;
use crate::location::{CodeGenInfo, CodeLocation};
use crate::name::{Sym, VPath};
use crate::parse::parsed;
use crate::pipeline::project::{
  collect_consts, collect_rules, ConstReport, ProjectTree,
};
use crate::rule::repository::Repo;
use crate::tree::ModMember;

/// Everything needed for macro execution, and constructing the process
pub struct PreMacro<'a> {
  /// Optimized catalog of substitution rules
  pub repo: Repo,
  /// Runtime code containing macro invocations
  pub consts: HashMap<Sym, (ConstReport, CodeLocation)>,
  /// Libraries and plug-ins
  pub systems: Vec<System<'a>>,
}
impl<'a> PreMacro<'a> {
  /// Build a [PreMacro] from a source tree and system list
  pub fn new(
    tree: &ProjectTree,
    systems: Vec<System<'a>>,
  ) -> ProjectResult<Self> {
    Ok(Self {
      repo,
      consts: (consts.into_iter())
        .map(|(name, expr)| {
          let (ent, _) = (tree.0)
            .walk1_ref(&[], &name.split_last().1[..], |_| true)
            .expect("path sourced from symbol names");
          let location = (ent.x.locations.first().cloned())
            .unwrap_or_else(|| CodeLocation::Source(expr.value.range.clone()));
          (name, (expr, location))
        })
        .collect(),
      systems,
    })
  }

  /// Run all macros to termination or the optional timeout. If a timeout does
  /// not occur, returns a process which can execute Orchid code
  pub fn run_macros(
    self,
    timeout: Option<usize>,
  ) -> ProjectResult<Process<'a>> {
    let Self { systems, repo, consts } = self;
    for sys in systems.iter() {
      let const_module = sys.constants.unwrap_mod_ref();
      let _ = const_module.search_all((), &mut |path, module, ()| {
        for (key, ent) in &module.entries {
          if let ModMember::Item(c) = &ent.member {
            let path = VPath::new(path.unreverse()).as_prefix_of(key.clone());
            let cginfo = CodeGenInfo::details(
              "constant from",
              format!("system.name={}", sys.name),
            );
            symbols
              .insert(path.to_sym(), c.gen_nort(CodeLocation::Gen(cginfo)));
          }
        }
        Ok::<(), Never>(())
      });
    }
    Ok(Process {
      symbols,
      handlers: (systems.into_iter())
        .fold(HandlerTable::new(), |tbl, sys| tbl.combine(sys.handlers)),
    })
  }

  /// Obtain an iterator that steps through the preprocessing of a constant
  /// for debugging macros
  pub fn step(&self, sym: Sym) -> impl Iterator<Item = parsed::Expr> + '_ {
    let mut target =
      self.consts.get(&sym).expect("Target not found").0.value.clone();
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
