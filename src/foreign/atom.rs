use std::any::Any;
use std::fmt::Debug;
use std::rc::Rc;

use dyn_clone::DynClone;

use super::ExternError;
use crate::ddispatch::request;
use crate::error::AssertionError;
use crate::interpreted::{ExprInst, TryFromExprInst};
use crate::interpreter::{Context, RuntimeError};
use crate::representations::interpreted::Clause;
use crate::utils::ddispatch::Responder;

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

/// Returned by [Atomic::run]
pub type AtomicResult = Result<AtomicReturn, RuntimeError>;

/// Trait for things that are _definitely_ equal.
pub trait StrictEq {
  /// must return true if the objects were produced via the exact same sequence
  /// of transformations, including any relevant context data. Must return false
  /// if the objects are of different type, or if their type is [PartialEq]
  /// and [PartialEq::eq] returns false.
  fn strict_eq(&self, other: &dyn Any) -> bool;
}

/// Functionality the interpreter needs to handle a value
pub trait Atomic: Any + Debug + DynClone + StrictEq + Responder + Send
where
  Self: 'static,
{
  /// Casts this value to [Any] so that its original value can be salvaged
  /// during introspection by other external code.
  ///
  /// This function should be implemented in exactly one way:
  ///
  /// ```ignore
  /// fn as_any(self: Box<Self>) -> Box<dyn Any> { self }
  /// ```
  #[must_use]
  fn as_any(self: Box<Self>) -> Box<dyn Any>;
  /// See [Atomic::as_any], exactly the same but for references
  #[must_use]
  fn as_any_ref(&self) -> &dyn Any;

  /// Attempt to normalize this value. If it wraps a value, this should report
  /// inert. If it wraps a computation, it should execute one logical step of
  /// the computation and return a structure representing the ntext.
  fn run(self: Box<Self>, ctx: Context) -> AtomicResult;

  /// Wrap the atom in a clause to be placed in an [AtomicResult].
  #[must_use]
  fn atom_cls(self) -> Clause
  where
    Self: Sized,
  {
    Clause::Atom(Atom(Box::new(self)))
  }

  /// Wrap the atom in a new expression instance to be placed in a tree
  #[must_use]
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
/// to pass control back to the interpreter.
pub struct Atom(pub Box<dyn Atomic>);
impl Atom {
  /// Wrap an [Atomic] in a type-erased box
  #[must_use]
  pub fn new<T: 'static + Atomic>(data: T) -> Self {
    Self(Box::new(data) as Box<dyn Atomic>)
  }
  /// Get the contained data
  #[must_use]
  pub fn data(&self) -> &dyn Atomic { self.0.as_ref() as &dyn Atomic }
  /// Test the type of the contained data without downcasting
  #[must_use]
  pub fn is<T: Atomic>(&self) -> bool { self.data().as_any_ref().is::<T>() }
  /// Downcast contained data, panic if it isn't the specified type
  #[must_use]
  pub fn downcast<T: Atomic>(self) -> T {
    *self.0.as_any().downcast().expect("Type mismatch on Atom::cast")
  }
  /// Normalize the contained data
  pub fn run(self, ctx: Context) -> AtomicResult { self.0.run(ctx) }
  /// Request a delegate from the encapsulated data
  pub fn request<T: 'static>(&self) -> Option<T> { request(self.0.as_ref()) }
  /// Downcast the atom to a concrete atomic type, or return the original atom
  /// if it is not the specified type
  pub fn try_downcast<T: Atomic>(self) -> Result<T, Self> {
    match self.0.as_any_ref().is::<T>() {
      true => Ok(*self.0.as_any().downcast().expect("checked just above")),
      false => Err(self),
    }
  }
}

impl Clone for Atom {
  fn clone(&self) -> Self { Self(dyn_clone::clone_box(self.data())) }
}

impl Debug for Atom {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self.data())
  }
}

impl TryFromExprInst for Atom {
  fn from_exi(exi: ExprInst) -> Result<Self, Rc<dyn ExternError>> {
    let loc = exi.location();
    match exi.expr_val().clause {
      Clause::Atom(a) => Ok(a),
      _ => AssertionError::fail(loc, "atom"),
    }
  }
}
