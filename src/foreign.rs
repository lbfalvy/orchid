//! Interaction with foreign code
//!
//! Structures and traits used in the exposure of external functions and values
//! to Orchid code
use std::any::Any;
use std::error::Error;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::rc::Rc;

use dyn_clone::DynClone;

use crate::interpreter::{Context, RuntimeError};
pub use crate::representations::interpreted::Clause;
use crate::representations::interpreted::ExprInst;
use crate::representations::Primitive;

/// Information returned by [Atomic::run]. This mirrors
/// [crate::interpreter::Return] but with a clause instead of an Expr.
pub struct AtomicReturn {
  pub clause: Clause,
  pub gas: Option<usize>,
  pub inert: bool,
}
impl AtomicReturn {
  /// Wrap an inert atomic for delivery to the supervisor
  pub fn from_data<D: Atomic>(d: D, c: Context) -> Self {
    AtomicReturn { clause: d.to_atom_cls(), gas: c.gas, inert: false }
  }
}

/// A type-erased error in external code
pub type RcError = Rc<dyn ExternError>;
/// Returned by [Atomic::run]
pub type AtomicResult = Result<AtomicReturn, RuntimeError>;
/// Returned by [ExternFn::apply]
pub type XfnResult = Result<Clause, RcError>;

/// Errors produced by external code
pub trait ExternError: Display {
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
  fn to_xfn_cls(self) -> Clause
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

/// Functionality the interpreter needs to handle a value
pub trait Atomic: Any + Debug + DynClone
where
  Self: 'static,
{
  /// Casts this value to [Any] so that its original value can be salvaged
  /// during introspection by other external code. There is no other way to
  /// interact with values of unknown types at the moment.
  fn as_any(&self) -> &dyn Any;
  /// Attempt to normalize this value. If it wraps a value, this should report
  /// inert. If it wraps a computation, it should execute one logical step of
  /// the computation and return a structure representing the ntext.
  fn run(&self, ctx: Context) -> AtomicResult;
  /// Wrap the atom in a clause to be placed in an [AtomicResult].
  fn to_atom_cls(self) -> Clause
  where
    Self: Sized,
  {
    Clause::P(Primitive::Atom(Atom(Box::new(self))))
  }
}

/// Represents a black box unit of code with its own normalization steps.
/// Typically [ExternFn] will produce an [Atom] when applied to a [Clause],
/// this [Atom] will then forward `run` calls to the argument until it becomes
/// inert at which point the [Atom] will validate and process the argument,
/// returning a different [Atom] intended for processing by external code, a new
/// [ExternFn] to capture an additional argument, or an Orchid expression
/// to pass control back to the interpreter.btop
pub struct Atom(pub Box<dyn Atomic>);
impl Atom {
  pub fn new<T: 'static + Atomic>(data: T) -> Self {
    Self(Box::new(data) as Box<dyn Atomic>)
  }
  pub fn data(&self) -> &dyn Atomic {
    self.0.as_ref() as &dyn Atomic
  }
  pub fn try_cast<T: Atomic>(&self) -> Option<&T> {
    self.data().as_any().downcast_ref()
  }
  pub fn is<T: 'static>(&self) -> bool {
    self.data().as_any().is::<T>()
  }
  pub fn cast<T: 'static>(&self) -> &T {
    self.data().as_any().downcast_ref().expect("Type mismatch on Atom::cast")
  }
  pub fn run(&self, ctx: Context) -> AtomicResult {
    self.0.run(ctx)
  }
}

impl Clone for Atom {
  fn clone(&self) -> Self {
    Self(dyn_clone::clone_box(self.data()))
  }
}

impl Debug for Atom {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "##ATOM[{:?}]##", self.data())
  }
}
