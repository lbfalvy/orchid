use std::error::Error;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::rc::Rc;

use dyn_clone::DynClone;

use crate::interpreted::ExprInst;
use crate::interpreter::Context;
use crate::representations::interpreted::Clause;
use crate::Primitive;

/// Returned by [ExternFn::apply]
pub type XfnResult = Result<Clause, Rc<dyn ExternError>>;

/// Errors produced by external code
pub trait ExternError: Display {
  /// Convert into trait object
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
pub trait ExternFn: DynClone {
  /// Display name of the function
  fn name(&self) -> &str;
  /// Combine the function with an argument to produce a new clause
  fn apply(&self, arg: ExprInst, ctx: Context) -> XfnResult;
  /// Hash the name to get a somewhat unique hash.
  fn hash(&self, mut state: &mut dyn std::hash::Hasher) {
    self.name().hash(&mut state)
  }
  /// Wrap this function in a clause to be placed in an [AtomicResult].
  fn xfn_cls(self) -> Clause
  where
    Self: Sized + 'static,
  {
    Clause::P(Primitive::ExternFn(Box::new(self)))
  }
}

impl Eq for dyn ExternFn {}
impl PartialEq for dyn ExternFn {
  fn eq(&self, other: &Self) -> bool {
    self.name() == other.name()
  }
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
