use super::atom::Atomic;
use crate::interpreter::nort::{Clause, ClauseInst, Expr};
use crate::location::CodeLocation;

/// A trait for things that are infallibly convertible to [ClauseInst]. These
/// types can be returned by callbacks passed to the [super::xfn_1ary] family of
/// functions.
pub trait ToClause: Sized {
  /// Convert this value to a [Clause]. If your value can only be directly
  /// converted to a [ClauseInst], you can call `ClauseInst::to_clause` to
  /// unwrap it if possible or fall back to [Clause::Identity].
  fn to_clause(self, location: CodeLocation) -> Clause;

  /// Convert the type to a [Clause].
  fn to_clsi(self, location: CodeLocation) -> ClauseInst {
    ClauseInst::new(self.to_clause(location))
  }

  /// Convert to an expression via [ToClause].
  fn to_expr(self, location: CodeLocation) -> Expr {
    Expr { clause: self.to_clsi(location.clone()), location }
  }
}

impl<T: Atomic + Clone> ToClause for T {
  fn to_clause(self, _: CodeLocation) -> Clause { self.atom_cls() }
}
impl ToClause for Clause {
  fn to_clause(self, _: CodeLocation) -> Clause { self }
}
impl ToClause for ClauseInst {
  fn to_clause(self, _: CodeLocation) -> Clause {
    self.into_cls()
  }
  fn to_clsi(self, _: CodeLocation) -> ClauseInst { self }
}
impl ToClause for Expr {
  fn to_clause(self, location: CodeLocation) -> Clause {
    self.clause.to_clause(location)
  }
  fn to_clsi(self, _: CodeLocation) -> ClauseInst { self.clause }
  fn to_expr(self, _: CodeLocation) -> Expr { self }
}
