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
#[allow(unused)] // for doc
use crate::foreign::Atomic;
use crate::foreign::ExternError;
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
    match &self.location {
      Location::Unknown => write!(f, "{}", self.clause),
      loc => write!(f, "{}:({})", loc, self.clause),
    }
  }
}

/// [ExprInst::with_literal] produces this marker unit to indicate that the
/// expression is not a literal
pub struct NotALiteral;

/// Types automatically convertible from an [ExprInst]
pub trait TryFromExprInst: Sized {
  /// Match and clone the value out of an [ExprInst]
  fn from_exi(exi: ExprInst) -> Result<Self, Rc<dyn ExternError>>;
}

impl TryFromExprInst for ExprInst {
  fn from_exi(exi: ExprInst) -> Result<Self, Rc<dyn ExternError>> { Ok(exi) }
}

/// A wrapper around expressions to handle their multiple occurences in
/// the tree together
#[derive(Clone)]
pub struct ExprInst(pub Rc<RefCell<Expr>>);
impl ExprInst {
  /// Wrap an [Expr] in a shared container so that normalizatoin steps are
  /// applied to all references
  #[must_use]
  pub fn new(expr: Expr) -> Self { Self(Rc::new(RefCell::new(expr))) }

  /// Take the [Expr] out of this container if it's the last reference to it, or
  /// clone it out.
  #[must_use]
  pub fn expr_val(self) -> Expr {
    Rc::try_unwrap(self.0)
      .map(|c| c.into_inner())
      .unwrap_or_else(|rc| rc.as_ref().borrow().deref().clone())
  }

  /// Read-only access to the shared expression instance
  ///
  /// # Panics
  ///
  /// if the expression is already borrowed in read-write mode
  #[must_use]
  pub fn expr(&self) -> impl Deref<Target = Expr> + '_ {
    self.0.as_ref().borrow()
  }

  /// Read-Write access to the shared expression instance
  ///
  /// # Panics
  ///
  /// if the expression is already borrowed
  #[must_use]
  pub fn expr_mut(&self) -> impl DerefMut<Target = Expr> + '_ {
    self.0.as_ref().borrow_mut()
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

  /// Call the predicate on the value inside this expression if it is a
  /// primitive
  pub fn get_literal(self) -> Result<(Literal, Location), Self> {
    Rc::try_unwrap(self.0).map_or_else(
      |rc| {
        if let Expr { clause: Clause::P(Primitive::Literal(li)), location } =
          rc.as_ref().borrow().deref()
        {
          return Ok((li.clone(), location.clone()));
        }
        Err(Self(rc))
      },
      |cell| match cell.into_inner() {
        Expr { clause: Clause::P(Primitive::Literal(li)), location } =>
          Ok((li, location)),
        expr => Err(Self::new(expr)),
      },
    )
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
      | Clause::P(_)
      | Clause::Bottom => None,
    })
  }

  /// Convert into any type that implements [FromExprInst]. Calls to this
  /// function are generated wherever a conversion is elided in an extern
  /// function.
  pub fn downcast<T: TryFromExprInst>(self) -> Result<T, Rc<dyn ExternError>> {
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
      Clause::P(Primitive::Atom(a)) => request(&*a.0),
      _ => None,
    }
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
  /// An expression that causes an error
  Bottom,
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
      Clause::P(p) => write!(f, "{p:?}"),
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

impl<T: Into<Literal>> From<T> for Clause {
  fn from(value: T) -> Self { Self::P(Primitive::Literal(value.into())) }
}

impl<T: Into<Clause>> From<T> for ExprInst {
  fn from(value: T) -> Self { value.into().wrap() }
}
