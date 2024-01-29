//! The NORT (Normal Order Referencing Tree) is the interpreter's runtime
//! representation of Orchid programs.
//!
//! It uses a locator tree to find bound variables in lambda functions, which
//! necessitates a normal reduction order because modifying the body by reducing
//! expressions would invalidate any locators in enclosing lambdas.
//!
//! Clauses are held in a mutable `Arc<Mutex<_>>`, so that after substitution
//! the instances of the argument remain linked and a reduction step applied to
//! any instance transforms all of them.
//!
//! To improve locality and make the tree less deep and locators shorter,
//! function calls store multiple arguments in a deque.

use std::collections::VecDeque;
use std::fmt::{Debug, Display};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, TryLockError};

use itertools::Itertools;

use super::error::RunError;
use super::path_set::PathSet;
use crate::foreign::atom::Atom;
#[allow(unused)] // for doc
use crate::foreign::atom::Atomic;
use crate::foreign::error::ExternResult;
use crate::foreign::try_from_expr::TryFromExpr;
use crate::location::CodeLocation;
use crate::name::Sym;
#[allow(unused)] // for doc
use crate::parse::parsed;
use crate::utils::ddispatch::request;
use crate::utils::take_with_output::take_with_output;

/// Kinda like [AsMut] except it supports a guard
pub(crate) trait AsDerefMut<T> {
  fn as_deref_mut(&mut self) -> impl DerefMut<Target = T> + '_;
}

/// An expression with metadata
#[derive(Clone)]
pub struct Expr {
  /// The actual value
  pub clause: ClauseInst,
  /// Information about the code that produced this value
  pub location: CodeLocation,
}
impl Expr {
  /// Constructor
  pub fn new(clause: ClauseInst, location: CodeLocation) -> Self {
    Self { clause, location }
  }
  /// Obtain the location of the expression
  pub fn location(&self) -> CodeLocation { self.location.clone() }

  /// Convert into any type that implements [TryFromExpr]. Calls to this
  /// function are generated wherever a conversion is elided in an extern
  /// function.
  pub fn downcast<T: TryFromExpr>(self) -> ExternResult<T> {
    let Expr { mut clause, location } = self;
    loop {
      let cls_deref = clause.cls();
      match &*cls_deref {
        Clause::Identity(alt) => {
          let temp = alt.clone();
          drop(cls_deref);
          clause = temp;
        },
        _ => {
          drop(cls_deref);
          return T::from_expr(Expr { clause, location });
        },
      };
    }
  }

  /// Visit all expressions in the tree. The search can be exited early by
  /// returning [Some]
  ///
  /// See also [parsed::Expr::search_all]
  pub fn search_all<T>(
    &self,
    predicate: &mut impl FnMut(&Self) -> Option<T>,
  ) -> Option<T> {
    if let Some(t) = predicate(self) {
      return Some(t);
    }
    self.clause.inspect(|c| match c {
      Clause::Identity(_alt) => unreachable!("Handled by inspect"),
      Clause::Apply { f, x } => (f.search_all(predicate))
        .or_else(|| x.iter().find_map(|x| x.search_all(predicate))),
      Clause::Lambda { body, .. } => body.search_all(predicate),
      Clause::Constant(_)
      | Clause::LambdaArg
      | Clause::Atom(_)
      | Clause::Bottom(_) => None,
    })
  }

  /// Clone the refcounted [ClauseInst] out of the expression
  #[must_use]
  pub fn clsi(&self) -> ClauseInst { self.clause.clone() }

  /// Readonly access to the [Clause]
  ///
  /// # Panics
  ///
  /// if the clause is already borrowed
  #[must_use]
  pub fn cls(&self) -> impl Deref<Target = Clause> + '_ { self.clause.cls() }

  /// Read-Write access to the [Clause]
  ///
  /// # Panics
  ///
  /// if the clause is already borrowed
  #[must_use]
  pub fn cls_mut(&self) -> impl DerefMut<Target = Clause> + '_ {
    self.clause.cls_mut()
  }
}

impl Debug for Expr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}@{}", self.clause, self.location)
  }
}

impl Display for Expr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.clause)
  }
}

impl AsDerefMut<Clause> for Expr {
  fn as_deref_mut(&mut self) -> impl DerefMut<Target = Clause> + '_ {
    self.clause.cls_mut()
  }
}

/// [ExprInst::with_literal] produces this marker unit to indicate that the
/// expression is not a literal
pub struct NotALiteral;

/// A wrapper around expressions to handle their multiple occurences in
/// the tree together
#[derive(Clone)]
pub struct ClauseInst(pub Arc<Mutex<Clause>>);
impl ClauseInst {
  /// Wrap a [Clause] in a shared container so that normalization steps are
  /// applied to all references
  #[must_use]
  pub fn new(cls: Clause) -> Self { Self(Arc::new(Mutex::new(cls))) }

  /// Take the [Clause] out of this container if it's the last reference to it,
  /// or return self.
  pub fn try_unwrap(self) -> Result<Clause, ClauseInst> {
    Arc::try_unwrap(self.0).map(|c| c.into_inner().unwrap()).map_err(Self)
  }

  /// Read-only access to the shared clause instance
  ///
  /// # Panics
  ///
  /// if the clause is already borrowed in read-write mode
  #[must_use]
  pub fn cls(&self) -> impl Deref<Target = Clause> + '_ {
    self.0.lock().unwrap()
  }

  /// Read-Write access to the shared clause instance
  ///
  /// # Panics
  ///
  /// if the clause is already borrowed
  #[must_use]
  pub fn cls_mut(&self) -> impl DerefMut<Target = Clause> + '_ {
    self.0.lock().unwrap()
  }

  /// Call a normalization function on the expression. The expr is
  /// updated with the new clause which affects all copies of it
  /// across the tree.
  ///
  /// This function bypasses and collapses identities, but calling it in a plain
  /// loop intermittently re-acquires the mutex, and looping inside of it breaks
  /// identity collapsing. [ClauseInst::try_normalize_trampoline] solves these
  /// problems.
  pub fn try_normalize<T>(
    &self,
    mapper: impl FnOnce(Clause) -> Result<(Clause, T), RunError>,
  ) -> Result<(Self, T), RunError> {
    enum Report<T> {
      Nested(ClauseInst, T),
      Plain(T),
    }
    let ret = take_with_output(&mut *self.cls_mut(), |clause| match &clause {
      // don't modify identities, instead update and return the nested clause
      Clause::Identity(alt) => match alt.try_normalize(mapper) {
        Ok((nested, t)) => (clause, Ok(Report::Nested(nested, t))),
        Err(e) => (Clause::Bottom(e.clone()), Err(e)),
      },
      _ => match mapper(clause) {
        Err(e) => (Clause::Bottom(e.clone()), Err(e)),
        Ok((clause, t)) => (clause, Ok(Report::Plain(t))),
      },
    })?;
    Ok(match ret {
      Report::Nested(nested, t) => (nested, t),
      Report::Plain(t) => (self.clone(), t),
    })
  }

  /// Repeatedly call a normalization function on the held clause, switching
  /// [ClauseInst] values as needed to ensure that
  pub fn try_normalize_trampoline<T>(
    mut self,
    mut mapper: impl FnMut(Clause) -> Result<(Clause, Option<T>), RunError>,
  ) -> Result<(Self, T), RunError> {
    loop {
      let (next, exit) = self.try_normalize(|mut cls| {
        loop {
          if matches!(cls, Clause::Identity(_)) {
            break Ok((cls, None));
          }
          let (next, exit) = mapper(cls)?;
          if let Some(exit) = exit {
            break Ok((next, Some(exit)));
          }
          cls = next;
        }
      })?;
      if let Some(exit) = exit {
        break Ok((next, exit));
      }
      self = next
    }
  }

  /// Call a predicate on the clause, returning whatever the
  /// predicate returns. This is a convenience function for reaching
  /// through the [Mutex]. The clause will never be [Clause::Identity].
  #[must_use]
  pub fn inspect<T>(&self, predicate: impl FnOnce(&Clause) -> T) -> T {
    match &*self.cls() {
      Clause::Identity(sub) => sub.inspect(predicate),
      x => predicate(x),
    }
  }

  /// If this expression is an [Atomic], request an object of the given type.
  /// If it's not an atomic, fail the request automatically.
  #[must_use = "your request might not have succeeded"]
  pub fn request<T: 'static>(&self) -> Option<T> {
    match &*self.cls() {
      Clause::Atom(a) => request(&*a.0),
      Clause::Identity(alt) => alt.request(),
      _ => None,
    }
  }

  /// Associate a location with this clause
  pub fn to_expr(self, location: CodeLocation) -> Expr {
    Expr { clause: self.clone(), location: location.clone() }
  }
  /// Check ahead-of-time if this clause contains an atom. Calls
  /// [ClauseInst#cls] for read access.
  ///
  /// Since atoms cannot become normalizable, if this is true and previous
  /// normalization failed, the atom is known to be in normal form.
  pub fn is_atom(&self) -> bool { matches!(&*self.cls(), Clause::Atom(_)) }

  /// Tries to unwrap the [Arc]. If that fails, clones it field by field.
  /// If it's a [Clause::Atom] which cannot be cloned, wraps it in a
  /// [Clause::Identity].
  ///
  /// Implementation of [crate::foreign::to_clause::ToClause::to_clause]. The
  /// trait is more general so it requires a location which this one doesn't.
  pub fn into_cls(self) -> Clause {
    self.try_unwrap().unwrap_or_else(|clsi| match &*clsi.cls() {
      Clause::Apply { f, x } => Clause::Apply { f: f.clone(), x: x.clone() },
      Clause::Atom(_) => Clause::Identity(clsi.clone()),
      Clause::Bottom(e) => Clause::Bottom(e.clone()),
      Clause::Constant(c) => Clause::Constant(c.clone()),
      Clause::Identity(sub) => Clause::Identity(sub.clone()),
      Clause::Lambda { args, body } =>
        Clause::Lambda { args: args.clone(), body: body.clone() },
      Clause::LambdaArg => Clause::LambdaArg,
    })
  }
}

impl Debug for ClauseInst {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self.0.try_lock() {
      Ok(expr) => write!(f, "{expr:?}"),
      Err(TryLockError::Poisoned(_)) => write!(f, "<poisoned>"),
      Err(TryLockError::WouldBlock) => write!(f, "<locked>"),
    }
  }
}

impl Display for ClauseInst {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self.0.try_lock() {
      Ok(expr) => write!(f, "{expr}"),
      Err(TryLockError::Poisoned(_)) => write!(f, "<poisoned>"),
      Err(TryLockError::WouldBlock) => write!(f, "<locked>"),
    }
  }
}

impl AsDerefMut<Clause> for ClauseInst {
  fn as_deref_mut(&mut self) -> impl DerefMut<Target = Clause> + '_ {
    self.cls_mut()
  }
}

/// Distinct types of expressions recognized by the interpreter
#[derive(Debug)]
pub enum Clause {
  /// An expression that causes an error
  Bottom(RunError),
  /// Indicates that this [ClauseInst] has the same value as the other
  /// [ClauseInst]. This has two benefits;
  ///
  /// - [Clause] and therefore [Atomic] doesn't have to be [Clone] which saves
  ///   many synchronization primitives and reference counters in usercode
  /// - it enforces on the type level that all copies are normalized together,
  ///   so accidental inefficiency in the interpreter is rarer.
  ///
  /// That being said, it's still arbitrary many indirections, so when possible
  /// APIs should be usable with a [ClauseInst] directly.
  Identity(ClauseInst),
  /// An opaque non-callable value, eg. a file handle
  Atom(Atom),
  /// A function application
  Apply {
    /// Function to be applied
    f: Expr,
    /// Argument to be substituted in the function
    x: VecDeque<Expr>,
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
    body: Expr,
  },
  /// A placeholder within a function that will be replaced upon application
  LambdaArg,
}
impl Clause {
  /// Wrap a clause in a refcounted lock
  pub fn to_inst(self) -> ClauseInst { ClauseInst::new(self) }
  /// Wrap a clause in an expression.
  pub fn to_expr(self, location: CodeLocation) -> Expr {
    self.to_inst().to_expr(location)
  }
}

impl Display for Clause {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Clause::Atom(a) => write!(f, "{a:?}"),
      Clause::Bottom(err) => write!(f, "bottom({err})"),
      Clause::LambdaArg => write!(f, "arg"),
      Clause::Apply { f: fun, x } =>
        write!(f, "({fun} {})", x.iter().join(" ")),
      Clause::Lambda { args, body } => match args {
        Some(path) => write!(f, "[\\{path}.{body}]"),
        None => write!(f, "[\\_.{body}]"),
      },
      Clause::Constant(t) => write!(f, "{t}"),
      Clause::Identity(other) => write!(f, "{{{other}}}"),
    }
  }
}

impl AsDerefMut<Clause> for Clause {
  fn as_deref_mut(&mut self) -> impl DerefMut<Target = Clause> + '_ { self }
}
