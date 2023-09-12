//! The interpreter's changing internal representation of the code at runtime
//!
//! This code may be generated to minimize the number of states external
//! functions have to define
use std::cell::RefCell;
use std::fmt::{Debug, Display};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

#[allow(unused)] // for doc
use super::ast;
use super::location::Location;
use super::path_set::PathSet;
use super::primitive::Primitive;
use super::Literal;
use crate::Sym;

/// An expression with metadata
pub struct Expr {
  /// The actual value
  pub clause: Clause,
  /// Information about the code that produced this value
  pub location: Location,
}

impl Debug for Expr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match &self.location {
      Location::Unknown => write!(f, "{:?}", self.clause),
      loc => write!(f, "{:?}@{}", self.clause, loc),
    }
  }
}

impl Display for Expr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match &self.location {
      Location::Unknown => write!(f, "{}", self.clause),
      loc => write!(f, "{}:({})", loc, self.clause),
    }
  }
}

/// [ExprInst::with_literal] produces this marker unit to indicate that the
/// expression is not a literal
pub struct NotALiteral;

/// A wrapper around expressions to handle their multiple occurences in
/// the tree together
#[derive(Clone)]
pub struct ExprInst(pub Rc<RefCell<Expr>>);
impl ExprInst {
  /// Read-only access to the shared expression instance
  ///
  /// # Panics
  ///
  /// if the expression is already borrowed in read-write mode
  pub fn expr(&self) -> impl Deref<Target = Expr> + '_ {
    self.0.as_ref().borrow()
  }

  /// Read-Write access to the shared expression instance
  ///
  /// # Panics
  ///
  /// if the expression is already borrowed
  pub fn expr_mut(&self) -> impl DerefMut<Target = Expr> + '_ {
    self.0.as_ref().borrow_mut()
  }

  /// Call a normalization function on the expression. The expr is
  /// updated with the new clause which affects all copies of it
  /// across the tree.
  pub fn try_normalize<T, E>(
    &self,
    mapper: impl FnOnce(&Clause, &Location) -> Result<(Clause, T), E>,
  ) -> Result<(Self, T), E> {
    let expr = self.expr();
    let (new_clause, extra) = mapper(&expr.clause, &expr.location)?;
    drop(expr);
    self.expr_mut().clause = new_clause;
    Ok((self.clone(), extra))
  }

  /// Run a mutation function on the expression, producing a new,
  /// distinct expression. The new expression shares location info with
  /// the original but is normalized independently.
  pub fn try_update<T, E>(
    &self,
    mapper: impl FnOnce(&Clause, &Location) -> Result<(Clause, T), E>,
  ) -> Result<(Self, T), E> {
    let expr = self.expr();
    let (clause, extra) = mapper(&expr.clause, &expr.location)?;
    let new_expr = Expr { clause, location: expr.location.clone() };
    Ok((Self(Rc::new(RefCell::new(new_expr))), extra))
  }

  /// Call a predicate on the expression, returning whatever the
  /// predicate returns. This is a convenience function for reaching
  /// through the RefCell.
  pub fn inspect<T>(&self, predicate: impl FnOnce(&Clause) -> T) -> T {
    predicate(&self.expr().clause)
  }

  /// Call the predicate on the value inside this expression if it is a
  /// primitive
  pub fn with_literal<T>(
    &self,
    predicate: impl FnOnce(&Literal) -> T,
  ) -> Result<T, NotALiteral> {
    let expr = self.expr();
    if let Clause::P(Primitive::Literal(l)) = &expr.clause {
      Ok(predicate(l))
    } else {
      Err(NotALiteral)
    }
  }

  /// Visit all expressions in the tree. The search can be exited early by
  /// returning [Some]
  ///
  /// See also [ast::Expr::search_all]
  pub fn search_all<T>(
    &self,
    predicate: &mut impl FnMut(&Self) -> Option<T>,
  ) -> Option<T> {
    if let Some(t) = predicate(self) {
      return Some(t);
    }
    self.inspect(|c| match c {
      Clause::Apply { f, x } =>
        f.search_all(predicate).or_else(|| x.search_all(predicate)),
      Clause::Lambda { body, .. } => body.search_all(predicate),
      Clause::Constant(_) | Clause::LambdaArg | Clause::P(_) => None,
    })
  }
}

impl Debug for ExprInst {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self.0.try_borrow() {
      Ok(expr) => write!(f, "{expr:?}"),
      Err(_) => write!(f, "<borrowed>"),
    }
  }
}

impl Display for ExprInst {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self.0.try_borrow() {
      Ok(expr) => write!(f, "{expr}"),
      Err(_) => write!(f, "<borrowed>"),
    }
  }
}

/// Distinct types of expressions recognized by the interpreter
#[derive(Debug, Clone)]
pub enum Clause {
  /// An unintrospectable unit
  P(Primitive),
  /// A function application
  Apply {
    /// Function to be applied
    f: ExprInst,
    /// Argument to be substituted in the function
    x: ExprInst,
  },
  /// A name to be looked up in the interpreter's symbol table
  Constant(Sym),
  /// A function
  Lambda {
    /// A collection of (zero or more) paths to placeholders belonging to this
    /// function
    args: Option<PathSet>,
    /// The tree produced by this function, with placeholders where the
    /// argument will go
    body: ExprInst,
  },
  /// A placeholder within a function that will be replaced upon application
  LambdaArg,
}
impl Clause {
  /// Wrap a constructed clause in an expression. Avoid using this to wrap
  /// copied or moved clauses as it does not have debug information and
  /// does not share a normalization cache list with them.
  pub fn wrap(self) -> ExprInst {
    ExprInst(Rc::new(RefCell::new(Expr {
      location: Location::Unknown,
      clause: self,
    })))
  }
}

impl Display for Clause {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Clause::P(p) => write!(f, "{p:?}"),
      Clause::LambdaArg => write!(f, "arg"),
      Clause::Apply { f: fun, x } => write!(f, "({fun} {x})"),
      Clause::Lambda { args, body } => match args {
        Some(path) => write!(f, "\\{path:?}.{body}"),
        None => write!(f, "\\_.{body}"),
      },
      Clause::Constant(t) => write!(f, "{}", t.extern_vec().join("::")),
    }
  }
}

impl<T: Into<Literal>> From<T> for Clause {
  fn from(value: T) -> Self {
    Self::P(Primitive::Literal(value.into()))
  }
}

impl<T: Into<Clause>> From<T> for ExprInst {
  fn from(value: T) -> Self {
    value.into().wrap()
  }
}
