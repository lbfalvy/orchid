use std::error::Error;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::rc::Rc;

use dyn_clone::{clone_box, DynClone};

use super::XfnResult;
use crate::interpreted::ExprInst;
use crate::interpreter::Context;
use crate::representations::interpreted::Clause;

/// Errors produced by external code
pub trait ExternError: Display {
  /// Convert into trait object
  #[must_use]
  fn into_extern(self) -> Rc<dyn ExternError>
  where
    Self: 'static + Sized,
  {
    Rc::new(self)
  }
}

impl Debug for dyn ExternError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{self}")
  }
}

impl Error for dyn ExternError {}

/// Represents an externally defined function from the perspective of
/// the executor. Since Orchid lacks basic numerical operations,
/// these are also external functions.
pub trait ExternFn: DynClone + Send {
  /// Display name of the function
  #[must_use]
  fn name(&self) -> &str;
  /// Combine the function with an argument to produce a new clause
  fn apply(self: Box<Self>, arg: ExprInst, ctx: Context) -> XfnResult<Clause>;
  /// Hash the name to get a somewhat unique hash.
  fn hash(&self, mut state: &mut dyn std::hash::Hasher) {
    self.name().hash(&mut state)
  }
  /// Wrap this function in a clause to be placed in an [AtomicResult].
  #[must_use]
  fn xfn_cls(self) -> Clause
  where
    Self: Sized + 'static,
  {
    Clause::ExternFn(ExFn(Box::new(self)))
  }
}

impl Eq for dyn ExternFn {}
impl PartialEq for dyn ExternFn {
  fn eq(&self, other: &Self) -> bool { self.name() == other.name() }
}
impl Hash for dyn ExternFn {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.name().hash(state)
  }
}
impl Debug for dyn ExternFn {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "##EXTERN[{}]##", self.name())
  }
}

/// Represents a black box function that can be applied to a [Clause] to produce
/// a new [Clause], typically an [Atom] representing external work, a new [ExFn]
/// to take additional arguments, or an Orchid tree to return control to the
/// interpreter
#[derive(Debug)]
pub struct ExFn(pub Box<dyn ExternFn + 'static>);
impl ExFn {
  /// Combine the function with an argument to produce a new clause
  pub fn apply(self, arg: ExprInst, ctx: Context) -> XfnResult<Clause> {
    self.0.apply(arg, ctx)
  }
}
impl Clone for ExFn {
  fn clone(&self) -> Self { Self(clone_box(self.0.as_ref())) }
}
