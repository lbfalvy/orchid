use std::cell::RefCell;
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use crate::interner::{Token, InternedDisplay};
use crate::utils::print_nname;

use super::Literal;
use super::location::Location;
use super::path_set::PathSet;
use super::primitive::Primitive;

// TODO: implement Debug, Eq and Hash with cycle detection

pub struct Expr {
  pub clause: Clause,
  pub location: Location,
}

impl Debug for Expr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match &self.location {
      Location::Unknown => write!(f, "{:?}", self.clause),
      loc => write!(f, "{:?}@{}", self.clause, loc)
    }
  }
}

impl InternedDisplay for Expr {
  fn fmt_i(&self, f: &mut std::fmt::Formatter<'_>, i: &crate::interner::Interner) -> std::fmt::Result {
    match &self.location {
      Location::Unknown => self.clause.fmt_i(f, i),
      loc => {
        write!(f, "{}:(", loc)?;
        self.clause.fmt_i(f, i)?;
        write!(f, ")")
      }
    }
  }
}

/// A wrapper around expressions to handle their multiple occurences in
/// the tree
#[derive(Clone)]
pub struct ExprInst(pub Rc<RefCell<Expr>>);
impl ExprInst {
  pub fn expr<'a>(&'a self) -> impl Deref<Target = Expr> + 'a {
    self.0.as_ref().borrow()
  }

  pub fn expr_mut<'a>(&'a self) -> impl DerefMut<Target = Expr> + 'a {
    self.0.as_ref().borrow_mut()
  }

  /// Call a normalization function on the expression. The expr is
  /// updated with the new clause which affects all copies of it
  /// across the tree.
  pub fn try_normalize<E>(&self,
    mapper: impl FnOnce(&Clause) -> Result<Clause, E>
  ) -> Result<Self, E> {
    let new_clause = mapper(&self.expr().clause)?;
    self.expr_mut().clause = new_clause;
    Ok(self.clone())
  }

  /// Run a mutation function on the expression, producing a new,
  /// distinct expression. The new expression shares location info with
  /// the original but is normalized independently.
  pub fn try_update<E>(&self,
    mapper: impl FnOnce(&Clause) -> Result<Clause, E>
  ) -> Result<Self, E> {
    let expr = self.expr();
    let new_expr = Expr{
      clause: mapper(&expr.clause)?,
      location: expr.location.clone(),
    };
    Ok(Self(Rc::new(RefCell::new(new_expr))))
  }

  /// Call a predicate on the expression, returning whatever the
  /// predicate returns. This is a convenience function for reaching
  /// through the RefCell.
  pub fn inspect<T>(&self, predicate: impl FnOnce(&Clause) -> T) -> T {
    predicate(&self.expr().clause)
  }

  pub fn with_literal<T>(&self,
    predicate: impl FnOnce(&Literal) -> T
  ) -> Result<T, ()> {
    let expr = self.expr();
    if let Clause::P(Primitive::Literal(l)) = &expr.clause {
      Ok(predicate(l))
    } else {Err(())}
  }
}

impl Debug for ExprInst {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self.0.try_borrow() {
      Ok(expr) => write!(f, "{:?}", expr),
      Err(_) => write!(f, "<borrowed>"),
    }
  }
}

impl InternedDisplay for ExprInst {
  fn fmt_i(&self, f: &mut std::fmt::Formatter<'_>, i: &crate::interner::Interner) -> std::fmt::Result {
    match self.0.try_borrow() {
      Ok(expr) => expr.fmt_i(f, i),
      Err(_) => write!(f, "<borrowed>")
    }
  }
}

#[derive(Debug, Clone)]
pub enum Clause {
  P(Primitive),
  Apply{
    f: ExprInst,
    x: ExprInst
  },
  Constant(Token<Vec<Token<String>>>),
  Lambda{
    args: Option<PathSet>,
    body: ExprInst
  },
  LambdaArg,
}
impl Clause {
  /// Wrap a constructed clause in an expression. Avoid using this to wrap
  /// copied or moved clauses as it does not have debug information and
  /// does not share a normalization cache list with them.
  pub fn wrap(self) -> ExprInst {
    ExprInst(Rc::new(RefCell::new(Expr{
      location: Location::Unknown,
      clause: self
    })))
  }
}

impl InternedDisplay for Clause {
  fn fmt_i(&self, f: &mut std::fmt::Formatter<'_>, i: &crate::interner::Interner) -> std::fmt::Result {
    match self {
      Clause::P(p) => write!(f, "{p:?}"),
      Clause::LambdaArg => write!(f, "arg"),
      Clause::Apply { f: fun, x } => {
        write!(f, "(")?;
        fun.fmt_i(f, i)?;
        write!(f, " ")?;
        x.fmt_i(f, i)?;
        write!(f, ")")
      }
      Clause::Lambda { args, body } => {
        write!(f, "\\")?;
        match args {
          Some(path) => write!(f, "{path:?}")?,
          None => write!(f, "_")?,
        }
        write!(f, ".")?;
        body.fmt_i(f, i)
      },
      Clause::Constant(t) => write!(f, "{}", print_nname(*t, i))
    }
  }
}

impl<T: Into<Literal>> From<T> for Clause {
  fn from(value: T) -> Self {
    Self::P(Primitive::Literal(value.into()))
  }
}