//! Referencing a constant that doesn't exist is a runtime error in Orchid, even
//! though this common error condition is usually caused by faulty macro
//! execution. This module constains functions to detect and raise these errors
//! eagerly.

use std::fmt;

use hashbrown::HashSet;
use trait_set::trait_set;

use crate::error::{ProjectError, Reporter};
use crate::interpreter::nort::{Clause, Expr};
use crate::location::{CodeGenInfo, CodeLocation};
use crate::name::Sym;
use crate::sym;

/// Start with a symbol
pub fn unbound_refs_sym<E: SubError>(
  symbol: Sym,
  location: CodeLocation,
  visited: &mut HashSet<Sym>,
  load: &mut impl FnMut(Sym, CodeLocation) -> Result<Expr, E>,
  reporter: &Reporter,
) {
  if visited.insert(symbol.clone()) {
    match load(symbol.clone(), location.clone()) {
      Err(error) => reporter.report(MissingSymbol { symbol, location, error }.pack()),
      Ok(expr) => unbound_refs_expr(expr, visited, load, reporter),
    }
  }
}

/// Find all unbound constant names in a snippet. This is mostly useful to
/// detect macro errors.
pub fn unbound_refs_expr<E: SubError>(
  expr: Expr,
  visited: &mut HashSet<Sym>,
  load: &mut impl FnMut(Sym, CodeLocation) -> Result<Expr, E>,
  reporter: &Reporter,
) {
  expr.search_all(&mut |s: &Expr| {
    if let Clause::Constant(symbol) = &*s.cls_mut() {
      unbound_refs_sym(symbol.clone(), s.location(), visited, load, reporter)
    }
    None::<()>
  });
}

/// Assert that the code contains no invalid references that reference missing
/// symbols. [Clause::Constant]s can be created procedurally, so this isn't a
/// total guarantee, more of a convenience.
pub fn validate_refs<E: SubError>(
  all_syms: HashSet<Sym>,
  reporter: &Reporter,
  load: &mut impl FnMut(Sym, CodeLocation) -> Result<Expr, E>,
) -> HashSet<Sym> {
  let mut visited = HashSet::new();
  for sym in all_syms {
    let location = CodeLocation::new_gen(CodeGenInfo::no_details(sym!(orchidlang::validate_refs)));
    unbound_refs_sym(sym, location, &mut visited, load, reporter);
  }
  visited
}

trait_set! {
  /// Any error the reference walker can package into a [MissingSymbol]
  pub trait SubError = fmt::Display + Clone + Send + Sync + 'static;
}

/// Information about a reproject failure
#[derive(Clone)]
pub struct MissingSymbol<E: SubError> {
  /// The error returned by the loader function. This is usually a ismple "not
  /// found", but with unusual setups it might provide some useful info.
  pub error: E,
  /// Location of the first reference to the missing symbol.
  pub location: CodeLocation,
  /// The symbol in question
  pub symbol: Sym,
}
impl<E: SubError> ProjectError for MissingSymbol<E> {
  const DESCRIPTION: &'static str = "A name not referring to a known symbol was found in the source after \
     macro execution. This can either mean that a symbol name was mistyped, or \
     that macro execution didn't correctly halt.";
  fn message(&self) -> String { format!("{}: {}", self.symbol, self.error) }
  fn one_position(&self) -> crate::location::CodeOrigin { self.location.origin() }
}

// struct MissingSymbols {
//   errors: Vec<ErrorPosition>,
// }
// impl ProjectError for MissingSymbols {
//   fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> {
// self.errors.iter().cloned() } }
