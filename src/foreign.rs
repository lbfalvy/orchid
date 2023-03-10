use std::any::Any;
use std::fmt::{Display, Debug};
use std::hash::Hash;
use std::rc::Rc;

use dyn_clone::DynClone;

use crate::representations::interpreted::{Clause, RuntimeError, InternalError};

pub trait ExternError: Display {
  fn into_extern(self) -> Rc<dyn ExternError> where Self: 'static + Sized {
    Rc::new(self)
  }
}

/// Represents an externally defined function from the perspective of the executor
/// Since Orchid lacks basic numerical operations, these are also external functions.
pub trait ExternFn: DynClone {
  fn name(&self) -> &str;
  fn apply(&self, arg: Clause) -> Result<Clause, Rc<dyn ExternError>>;
  fn hash(&self, state: &mut dyn std::hash::Hasher) { state.write_str(self.name()) }
}

impl Eq for dyn ExternFn {}
impl PartialEq for dyn ExternFn {
  fn eq(&self, other: &Self) -> bool { self.name() == other.name() }
}
impl Hash for dyn ExternFn {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) { self.name().hash(state) }
}
impl Debug for dyn ExternFn {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "##EXTERN[{}]##", self.name())
  }
}

pub trait Atomic: Any + Debug + DynClone where Self: 'static {
  fn as_any(&self) -> &dyn Any;
  fn definitely_eq(&self, _other: &dyn Any) -> bool;
  fn hash(&self, hasher: &mut dyn std::hash::Hasher);
  fn run_once(&self) -> Result<Clause, InternalError>;
  fn run_n_times(&self, n: usize) -> Result<(Clause, usize), RuntimeError>;
  fn run_to_completion(&self) -> Result<Clause, RuntimeError>;
  fn typestr(&self) -> &str { "clause" }
}

/// Represents a black box unit of code with its own normalization steps. Typically [ExternFn]
/// will produce an [Atom] when applied to a [Clause], this [Atom] will then forward `run_*` calls
/// to the argument until it yields [InternalError::NonReducible] at which point the [Atom] will
/// validate and process the argument, returning a different [Atom] intended for processing by
/// external code, a new [ExternFn] to capture an additional argument, or an Orchid expression
/// to pass control back to the interpreter.
pub struct Atom(pub Box<dyn Atomic>);
impl Atom {
  pub fn new<T: 'static + Atomic>(data: T) -> Self {
    Self(Box::new(data) as Box<dyn Atomic>)
  }
  pub fn data(&self) -> &dyn Atomic { self.0.as_ref() as &dyn Atomic }
  pub fn try_cast<T: Atomic>(&self) -> Result<&T, ()> {
    self.data().as_any().downcast_ref().ok_or(())
  }
  pub fn is<T: 'static>(&self) -> bool { self.data().as_any().is::<T>() }
  pub fn cast<T: 'static>(&self) -> &T {
    self.data().as_any().downcast_ref().expect("Type mismatch on Atom::cast")
  }
}

impl Clone for Atom {
  fn clone(&self) -> Self {
    Self(dyn_clone::clone_box(self.data()))
  }
}

impl Hash for Atom {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.0.hash(state)
  }
}
impl Debug for Atom {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "##ATOM[{:?}]:{:?}##", self.data(), self.data().typestr())
  }
}
impl Eq for Atom {}
impl PartialEq for Atom {
  fn eq(&self, other: &Self) -> bool {
    self.data().definitely_eq(other.data().as_any())
  }
}