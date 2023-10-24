//! The interpreter's changing internal representation of the code at runtime
//!
//! This code may be generated to minimize the number of states external
//! functions have to define
use std::fmt::{Debug, Display};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, TryLockError};

#[allow(unused)] // for doc
use super::ast;
use super::location::Location;
use super::path_set::PathSet;
#[allow(unused)] // for doc
use crate::foreign::Atomic;
use crate::foreign::{Atom, ExFn, XfnResult};
use crate::utils::ddispatch::request;
use crate::utils::take_with_output;
use crate::Sym;

/// An expression with metadata
#[derive(Clone)]
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
    write!(f, "{}", self.clause)
    // match &self.location {
    //   Location::Unknown => write!(f, "{}", self.clause),
    //   loc => write!(f, "{}:({})", loc, self.clause),
    // }
  }
}

/// [ExprInst::with_literal] produces this marker unit to indicate that the
/// expression is not a literal
pub struct NotALiteral;

/// Types automatically convertible from an [ExprInst]
pub trait TryFromExprInst: Sized {
  /// Match and clone the value out of an [ExprInst]
  fn from_exi(exi: ExprInst) -> XfnResult<Self>;
}

impl TryFromExprInst for ExprInst {
  fn from_exi(exi: ExprInst) -> XfnResult<Self> { Ok(exi) }
}

/// A wrapper around expressions to handle their multiple occurences in
/// the tree together
#[derive(Clone)]
pub struct ExprInst(pub Arc<Mutex<Expr>>);
impl ExprInst {
  /// Wrap an [Expr] in a shared container so that normalizatoin steps are
  /// applied to all references
  #[must_use]
  pub fn new(expr: Expr) -> Self { Self(Arc::new(Mutex::new(expr))) }

  /// Take the [Expr] out of this container if it's the last reference to it, or
  /// clone it out.
  #[must_use]
  pub fn expr_val(self) -> Expr {
    Arc::try_unwrap(self.0)
      .map(|c| c.into_inner().unwrap())
      .unwrap_or_else(|arc| arc.lock().unwrap().clone())
  }

  /// Read-only access to the shared expression instance
  ///
  /// # Panics
  ///
  /// if the expression is already borrowed in read-write mode
  #[must_use]
  pub fn expr(&self) -> impl Deref<Target = Expr> + '_ {
    self.0.lock().unwrap()
  }

  /// Read-Write access to the shared expression instance
  ///
  /// # Panics
  ///
  /// if the expression is already borrowed
  #[must_use]
  pub fn expr_mut(&self) -> impl DerefMut<Target = Expr> + '_ {
    self.0.lock().unwrap()
  }

  /// Call a normalization function on the expression. The expr is
  /// updated with the new clause which affects all copies of it
  /// across the tree.
  pub fn try_normalize<T, E>(
    &self,
    mapper: impl FnOnce(Clause, &Location) -> Result<(Clause, T), E>,
  ) -> Result<(Self, T), E> {
    let extra = take_with_output(&mut *self.expr_mut(), |expr| {
      let Expr { clause, location } = expr;
      match mapper(clause, &location) {
        Ok((clause, t)) => (Expr { clause, location }, Ok(t)),
        Err(e) => (Expr { clause: Clause::Bottom, location }, Err(e)),
      }
    })?;
    Ok((self.clone(), extra))
  }

  /// Run a mutation function on the expression, producing a new,
  /// distinct expression. The new expression shares location info with
  /// the original but is normalized independently.
  pub fn try_update<T, E>(
    self,
    mapper: impl FnOnce(Clause, Location) -> Result<(Clause, T), E>,
  ) -> Result<(Self, T), E> {
    let Expr { clause, location } = self.expr_val();
    let (clause, extra) = mapper(clause, location.clone())?;
    Ok((Self::new(Expr { clause, location }), extra))
  }

  /// Call a predicate on the expression, returning whatever the
  /// predicate returns. This is a convenience function for reaching
  /// through the RefCell.
  #[must_use]
  pub fn inspect<T>(&self, predicate: impl FnOnce(&Clause) -> T) -> T {
    predicate(&self.expr().clause)
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
      Clause::Constant(_)
      | Clause::LambdaArg
      | Clause::Atom(_)
      | Clause::ExternFn(_)
      | Clause::Bottom => None,
    })
  }

  /// Convert into any type that implements [FromExprInst]. Calls to this
  /// function are generated wherever a conversion is elided in an extern
  /// function.
  pub fn downcast<T: TryFromExprInst>(self) -> XfnResult<T> {
    T::from_exi(self)
  }

  /// Get the code location data associated with this expresssion directly
  #[must_use]
  pub fn location(&self) -> Location { self.expr().location.clone() }

  /// If this expression is an [Atomic], request an object of the given type.
  /// If it's not an atomic, fail the request automatically.
  #[must_use = "your request might not have succeeded"]
  pub fn request<T: 'static>(&self) -> Option<T> {
    match &self.expr().clause {
      Clause::Atom(a) => request(&*a.0),
      _ => None,
    }
  }
}

impl Debug for ExprInst {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self.0.try_lock() {
      Ok(expr) => write!(f, "{expr:?}"),
      Err(TryLockError::Poisoned(_)) => write!(f, "<poisoned>"),
      Err(TryLockError::WouldBlock) => write!(f, "<locked>"),
    }
  }
}

impl Display for ExprInst {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self.0.try_lock() {
      Ok(expr) => write!(f, "{expr}"),
      Err(TryLockError::Poisoned(_)) => write!(f, "<poisoned>"),
      Err(TryLockError::WouldBlock) => write!(f, "<locked>"),
    }
  }
}

/// Distinct types of expressions recognized by the interpreter
#[derive(Debug, Clone)]
pub enum Clause {
  /// An expression that causes an error
  Bottom,
  /// An opaque function, eg. an effectful function employing CPS
  ExternFn(ExFn),
  /// An opaque non-callable value, eg. a file handle
  Atom(Atom),
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
    ExprInst(Arc::new(Mutex::new(Expr {
      location: Location::Unknown,
      clause: self,
    })))
  }

  /// Construct an application step
  pub fn apply(f: Self, x: Self) -> Self {
    Self::Apply { f: f.wrap(), x: x.wrap() }
  }

  /// Construct a lambda that uses its argument. See also [Clause::constfn]
  pub fn lambda(arg: PathSet, body: Self) -> Self {
    Self::Lambda { args: Some(arg), body: body.wrap() }
  }

  /// Construct a lambda that discards its argument. See also [Clause::lambda]
  pub fn constfn(body: Self) -> Self {
    Self::Lambda { args: None, body: body.wrap() }
  }

  /// Construct a lambda that picks its argument and places it in a directly
  /// descendant slot. Body must be a [Clause::LambdaArg] nested in an arbitrary
  /// number of [Clause::Lambda]s
  pub fn pick(body: Self) -> Self {
    Self::Lambda { args: Some(PathSet::pick()), body: body.wrap() }
  }
}

impl Display for Clause {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Clause::ExternFn(fun) => write!(f, "{fun:?}"),
      Clause::Atom(a) => write!(f, "{a:?}"),
      Clause::Bottom => write!(f, "bottom"),
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
