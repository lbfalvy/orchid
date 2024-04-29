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
use std::fmt;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

use itertools::Itertools;

use super::path_set::PathSet;
use crate::foreign::atom::Atom;
#[allow(unused)] // for doc
use crate::foreign::atom::Atomic;
use crate::foreign::error::{RTErrorObj, RTResult};
use crate::foreign::try_from_expr::TryFromExpr;
use crate::location::CodeLocation;
use crate::name::Sym;
#[allow(unused)] // for doc
use crate::parse::parsed;
use crate::utils::ddispatch::request;

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
  pub fn new(clause: ClauseInst, location: CodeLocation) -> Self { Self { clause, location } }
  /// Obtain the location of the expression
  pub fn location(&self) -> CodeLocation { self.location.clone() }

  /// Convert into any type that implements [TryFromExpr]. Calls to this
  /// function are generated wherever a conversion is elided in an extern
  /// function.
  pub fn downcast<T: TryFromExpr>(self) -> RTResult<T> {
    let Expr { mut clause, location } = self;
    loop {
      let cls_deref = clause.cls_mut();
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
  pub fn search_all<T>(&self, predicate: &mut impl FnMut(&Self) -> Option<T>) -> Option<T> {
    if let Some(t) = predicate(self) {
      return Some(t);
    }
    self.clause.inspect(|c| match c {
      Clause::Identity(_alt) => unreachable!("Handled by inspect"),
      Clause::Apply { f, x } =>
        (f.search_all(predicate)).or_else(|| x.iter().find_map(|x| x.search_all(predicate))),
      Clause::Lambda { body, .. } => body.search_all(predicate),
      Clause::Constant(_) | Clause::LambdaArg | Clause::Atom(_) | Clause::Bottom(_) => None,
    })
  }

  /// Clone the refcounted [ClauseInst] out of the expression
  #[must_use]
  pub fn clsi(&self) -> ClauseInst { self.clause.clone() }

  /// Read-Write access to the [Clause]
  ///
  /// # Panics
  ///
  /// if the clause is already borrowed
  pub fn cls_mut(&self) -> MutexGuard<'_, Clause> { self.clause.cls_mut() }
}

impl fmt::Debug for Expr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}@{}", self.clause, self.location)
  }
}

impl fmt::Display for Expr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.clause) }
}

impl AsDerefMut<Clause> for Expr {
  fn as_deref_mut(&mut self) -> impl DerefMut<Target = Clause> + '_ { self.clause.cls_mut() }
}

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

  /// Read-Write access to the shared clause instance
  ///
  /// if the clause is already borrowed, this will block until it is released.
  pub fn cls_mut(&self) -> MutexGuard<'_, Clause> { self.0.lock().unwrap() }

  /// Call a predicate on the clause, returning whatever the
  /// predicate returns. This is a convenience function for reaching
  /// through the [Mutex]. The clause will never be [Clause::Identity].
  #[must_use]
  pub fn inspect<T>(&self, predicate: impl FnOnce(&Clause) -> T) -> T {
    match &*self.cls_mut() {
      Clause::Identity(sub) => sub.inspect(predicate),
      x => predicate(x),
    }
  }

  /// If this expression is an [Atomic], request an object of the given type.
  /// If it's not an atomic, fail the request automatically.
  #[must_use = "your request might not have succeeded"]
  pub fn request<T: 'static>(&self) -> Option<T> {
    match &*self.cls_mut() {
      Clause::Atom(a) => request(&*a.0),
      Clause::Identity(alt) => alt.request(),
      _ => None,
    }
  }

  /// Associate a location with this clause
  pub fn into_expr(self, location: CodeLocation) -> Expr {
    Expr { clause: self.clone(), location: location.clone() }
  }
  /// Check ahead-of-time if this clause contains an atom. Calls
  /// [ClauseInst#cls] for read access.
  ///
  /// Since atoms cannot become normalizable, if this is true and previous
  /// normalization failed, the atom is known to be in normal form.
  pub fn is_atom(&self) -> bool { matches!(&*self.cls_mut(), Clause::Atom(_)) }

  /// Tries to unwrap the [Arc]. If that fails, clones it field by field.
  /// If it's a [Clause::Atom] which cannot be cloned, wraps it in a
  /// [Clause::Identity].
  ///
  /// Implementation of [crate::foreign::to_clause::ToClause::to_clause]. The
  /// trait is more general so it requires a location which this one doesn't.
  pub fn into_cls(self) -> Clause {
    self.try_unwrap().unwrap_or_else(|clsi| match &*clsi.cls_mut() {
      Clause::Apply { f, x } => Clause::Apply { f: f.clone(), x: x.clone() },
      Clause::Atom(_) => Clause::Identity(clsi.clone()),
      Clause::Bottom(e) => Clause::Bottom(e.clone()),
      Clause::Constant(c) => Clause::Constant(c.clone()),
      Clause::Identity(sub) => Clause::Identity(sub.clone()),
      Clause::Lambda { args, body } => Clause::Lambda { args: args.clone(), body: body.clone() },
      Clause::LambdaArg => Clause::LambdaArg,
    })
  }

  /// Decides if this clause is the exact same instance as another. Most useful
  /// to detect potential deadlocks.
  pub fn is_same(&self, other: &Self) -> bool { Arc::ptr_eq(&self.0, &other.0) }
}

impl fmt::Debug for ClauseInst {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self.0.try_lock() {
      Ok(expr) => write!(f, "{expr:?}"),
      Err(TryLockError::Poisoned(_)) => write!(f, "<poisoned>"),
      Err(TryLockError::WouldBlock) => write!(f, "<locked>"),
    }
  }
}

impl fmt::Display for ClauseInst {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self.0.try_lock() {
      Ok(expr) => write!(f, "{expr}"),
      Err(TryLockError::Poisoned(_)) => write!(f, "<poisoned>"),
      Err(TryLockError::WouldBlock) => write!(f, "<locked>"),
    }
  }
}

impl AsDerefMut<Clause> for ClauseInst {
  fn as_deref_mut(&mut self) -> impl DerefMut<Target = Clause> + '_ { self.cls_mut() }
}

/// Distinct types of expressions recognized by the interpreter
#[derive(Debug)]
pub enum Clause {
  /// An expression that causes an error
  Bottom(RTErrorObj),
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
  pub fn into_inst(self) -> ClauseInst { ClauseInst::new(self) }
  /// Wrap a clause in an expression.
  pub fn into_expr(self, location: CodeLocation) -> Expr { self.into_inst().into_expr(location) }
}

impl fmt::Display for Clause {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Clause::Atom(a) => write!(f, "{a:?}"),
      Clause::Bottom(err) => write!(f, "bottom({err})"),
      Clause::LambdaArg => write!(f, "arg"),
      Clause::Apply { f: fun, x } => write!(f, "({fun} {})", x.iter().join(" ")),
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
