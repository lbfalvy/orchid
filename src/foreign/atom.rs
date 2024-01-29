use std::any::Any;
use std::fmt::{Debug, Display};
use std::sync::{Arc, Mutex};

use never::Never;

use super::error::{ExternError, ExternResult};
use crate::interpreter::context::RunContext;
use crate::interpreter::error::RunError;
use crate::interpreter::nort;
use crate::location::{CodeLocation, SourceRange};
use crate::name::NameLike;
use crate::parse::parsed;
use crate::utils::ddispatch::{request, Request, Responder};

/// Information returned by [Atomic::run].
pub enum AtomicReturn {
  /// No work was done. If the atom takes an argument, it can be provided now
  Inert(nort::Clause),
  /// Work was done, returns new clause and consumed gas. 1 gas is already
  /// consumed by the virtual call, so nonzero values indicate expensive
  /// operations.
  Change(usize, nort::Clause),
}
impl AtomicReturn {
  /// Report indicating that the value is inert
  pub fn inert<T: Atomic, E>(this: T) -> Result<Self, E> {
    Ok(Self::Inert(this.atom_cls()))
  }
}

/// Returned by [Atomic::run]
pub type AtomicResult = Result<AtomicReturn, RunError>;

/// General error produced when a non-function [Atom] is applied to something as
/// a function.
#[derive(Clone)]
pub struct NotAFunction(pub nort::Expr);
impl ExternError for NotAFunction {}
impl Display for NotAFunction {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?} is not a function", self.0)
  }
}

/// Information about a function call presented to an external function
pub struct CallData<'a> {
  /// Location of the function expression
  pub location: CodeLocation,
  /// The argument the function was called on. Functions are curried
  pub arg: nort::Expr,
  /// Information relating to this interpreter run
  pub ctx: RunContext<'a>,
}

/// Information about a normalization run presented to an atom
#[derive(Clone)]
pub struct RunData<'a> {
  /// Location of the atom
  pub location: CodeLocation,
  /// Information about the execution
  pub ctx: RunContext<'a>,
}

/// Functionality the interpreter needs to handle a value
///
/// # Lifecycle methods
///
/// Atomics expose the methods [Atomic::redirect], [Atomic::run],
/// [Atomic::apply] and [Atomic::apply_ref] to interact with the interpreter.
/// The interpreter first tries to call `redirect` to find a subexpression to
/// normalize. If it returns `None` or the subexpression is inert, `run` is
/// called. `run` takes ownership of the value and returns a new one.
///
/// If `run` indicated in its return value that the result is inert and the atom
/// is in the position of a function, `apply` or `apply_ref` is called depending
/// upon whether the atom is referenced elsewhere. `apply` falls back to
/// `apply_ref` so implementing it is considered an optimization to avoid
/// excessive copying.
///
/// Atoms don't generally have to be copyable because clauses are refcounted in
/// the interpreter, but Orchid code is always free to duplicate the references
/// and apply them as functions to multiple different arguments so atoms that
/// represent functions have to support application by-ref without consuming the
/// function itself.
pub trait Atomic: Any + Debug + Responder + Send
where Self: 'static
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

  /// Returns a reference to a possible expression held inside the atom which
  /// can be reduced. For an overview of the lifecycle see [Atomic]
  fn redirect(&mut self) -> Option<&mut nort::Expr>;

  /// Attempt to normalize this value. If it wraps a value, this should report
  /// inert. If it wraps a computation, it should execute one logical step of
  /// the computation and return a structure representing the next.
  ///
  /// For an overview of the lifecycle see [Atomic]
  fn run(self: Box<Self>, run: RunData) -> AtomicResult;

  /// Combine the function with an argument to produce a new clause. Falls back
  /// to [Atomic::apply_ref] by default.
  ///
  /// For an overview of the lifecycle see [Atomic]
  fn apply(self: Box<Self>, call: CallData) -> ExternResult<nort::Clause> {
    self.apply_ref(call)
  }

  /// Combine the function with an argument to produce a new clause
  ///
  /// For an overview of the lifecycle see [Atomic]
  fn apply_ref(&self, call: CallData) -> ExternResult<nort::Clause>;

  /// Must return true for atoms parsed from identical source.
  /// If the atom cannot be parsed from source, it can safely be ignored
  #[allow(unused_variables)]
  fn parser_eq(&self, other: &dyn Any) -> bool { false }

  /// Wrap the atom in a clause to be placed in an [AtomicResult].
  #[must_use]
  fn atom_cls(self) -> nort::Clause
  where Self: Sized {
    nort::Clause::Atom(Atom(Box::new(self)))
  }

  /// Shorthand for `self.atom_cls().to_inst()`
  fn atom_clsi(self) -> nort::ClauseInst
  where Self: Sized {
    self.atom_cls().to_inst()
  }

  /// Wrap the atom in a new expression instance to be placed in a tree
  #[must_use]
  fn atom_expr(self, location: CodeLocation) -> nort::Expr
  where Self: Sized {
    self.atom_clsi().to_expr(location)
  }

  /// Wrap the atom in a clause to be placed in a [sourcefile::FileEntry].
  #[must_use]
  fn ast_cls(self) -> parsed::Clause
  where Self: Sized + Clone {
    parsed::Clause::Atom(AtomGenerator::cloner(self))
  }

  /// Wrap the atom in an expression to be placed in a [sourcefile::FileEntry].
  #[must_use]
  fn ast_exp<N: NameLike>(self, range: SourceRange) -> parsed::Expr
  where Self: Sized + Clone {
    self.ast_cls().into_expr(range)
  }
}

/// A struct for generating any number of [Atom]s. Since atoms aren't Clone,
/// this represents the ability to create any number of instances of an atom
#[derive(Clone)]
pub struct AtomGenerator(Arc<dyn Fn() -> Atom + Send + Sync>);
impl AtomGenerator {
  /// Use a factory function to create any number of atoms
  pub fn new(f: impl Fn() -> Atom + Send + Sync + 'static) -> Self {
    Self(Arc::new(f))
  }
  /// Clone a representative atom when called
  pub fn cloner(atom: impl Atomic + Clone) -> Self {
    let lock = Mutex::new(atom);
    Self::new(move || Atom::new(lock.lock().unwrap().clone()))
  }
  /// Generate an atom
  pub fn run(&self) -> Atom { self.0() }
}
impl Debug for AtomGenerator {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self.run())
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
  pub fn run(self, run: RunData) -> AtomicResult { self.0.run(run) }
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
  /// Downcast an atom by reference
  pub fn downcast_ref<T: Atomic>(&self) -> Option<&T> {
    self.0.as_any_ref().downcast_ref()
  }
  /// Combine the function with an argument to produce a new clause
  pub fn apply(self, call: CallData) -> ExternResult<nort::Clause> {
    self.0.apply(call)
  }
  /// Combine the function with an argument to produce a new clause
  pub fn apply_ref(&self, call: CallData) -> ExternResult<nort::Clause> {
    self.0.apply_ref(call)
  }
}

impl Debug for Atom {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self.data())
  }
}

impl Responder for Never {
  fn respond(&self, _request: Request) { match *self {} }
}
impl Atomic for Never {
  fn as_any(self: Box<Self>) -> Box<dyn Any> { match *self {} }
  fn as_any_ref(&self) -> &dyn Any { match *self {} }
  fn redirect(&mut self) -> Option<&mut nort::Expr> { match *self {} }
  fn run(self: Box<Self>, _: RunData) -> AtomicResult { match *self {} }
  fn apply_ref(&self, _: CallData) -> ExternResult<nort::Clause> {
    match *self {}
  }
}
