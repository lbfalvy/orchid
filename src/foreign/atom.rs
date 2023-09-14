use std::any::Any;
use std::fmt::Debug;

use dyn_clone::DynClone;

use crate::interpreted::ExprInst;
use crate::interpreter::{Context, RuntimeError};
use crate::representations::interpreted::Clause;
use crate::Primitive;

/// Information returned by [Atomic::run]. This mirrors
/// [crate::interpreter::Return] but with a clause instead of an Expr.
pub struct AtomicReturn {
  /// The next form of the expression
  pub clause: Clause,
  /// Remaining gas
  pub gas: Option<usize>,
  /// Whether further normalization is possible by repeated calls to
  /// [Atomic::run]
  pub inert: bool,
}
impl AtomicReturn {
  /// Wrap an inert atomic for delivery to the supervisor
  pub fn from_data<D: Atomic>(d: D, c: Context) -> Self {
    AtomicReturn { clause: d.atom_cls(), gas: c.gas, inert: false }
  }
}

/// Returned by [Atomic::run]
pub type AtomicResult = Result<AtomicReturn, RuntimeError>;

/// Functionality the interpreter needs to handle a value
pub trait Atomic: Any + Debug + DynClone
where
  Self: 'static,
{
  /// A fully type-erased interface to issue a command to the unknown type
  /// and see if it supports it
  fn request(&self, _request: Box<dyn Any>) -> Option<Box<dyn Any>> {
    None
  }

  /// Casts this value to [Any] so that its original value can be salvaged
  /// during introspection by other external code. There is no other way to
  /// interact with values of unknown types at the moment.
  fn as_any(&self) -> &dyn Any;

  /// Attempt to normalize this value. If it wraps a value, this should report
  /// inert. If it wraps a computation, it should execute one logical step of
  /// the computation and return a structure representing the ntext.
  fn run(&self, ctx: Context) -> AtomicResult;

  /// Wrap the atom in a clause to be placed in an [AtomicResult].
  fn atom_cls(self) -> Clause
  where
    Self: Sized,
  {
    Clause::P(Primitive::Atom(Atom(Box::new(self))))
  }

  /// Wrap the atom in a new expression instance to be placed in a tree
  fn atom_exi(self) -> ExprInst
  where
    Self: Sized,
  {
    self.atom_cls().wrap()
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
  /// Wrap an [Atomic] in a type-erased box
  pub fn new<T: 'static + Atomic>(data: T) -> Self {
    Self(Box::new(data) as Box<dyn Atomic>)
  }
  /// Get the contained data
  pub fn data(&self) -> &dyn Atomic {
    self.0.as_ref() as &dyn Atomic
  }
  /// Attempt to downcast contained data to a specific type
  pub fn try_cast<T: Atomic>(&self) -> Option<&T> {
    self.data().as_any().downcast_ref()
  }
  /// Test the type of the contained data without downcasting
  pub fn is<T: 'static>(&self) -> bool {
    self.data().as_any().is::<T>()
  }
  /// Downcast contained data, panic if it isn't the specified type
  pub fn cast<T: 'static>(&self) -> &T {
    self.data().as_any().downcast_ref().expect("Type mismatch on Atom::cast")
  }
  /// Normalize the contained data
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
