//! Abstractions for handling various code-related errors under a common trait
//! object.

use std::any::Any;
use std::cell::RefCell;
use std::sync::Arc;
use std::{fmt, process};

use dyn_clone::{clone_box, DynClone};
use itertools::Itertools;

use crate::location::CodeOrigin;
use crate::utils::boxed_iter::{box_once, BoxedIter};
#[allow(unused)] // for doc
use crate::virt_fs::CodeNotFound;

/// A point of interest in resolving the error, such as the point where
/// processing got stuck, a command that is likely to be incorrect
#[derive(Clone)]
pub struct ErrorPosition {
  /// The suspected origin
  pub origin: CodeOrigin,
  /// Any information about the role of this origin
  pub message: Option<String>,
}
impl From<CodeOrigin> for ErrorPosition {
  fn from(origin: CodeOrigin) -> Self { Self { origin, message: None } }
}

/// Errors addressed to the developer which are to be resolved with
/// code changes
pub trait ProjectError: Sized + Send + Sync + 'static {
  /// A general description of this type of error
  const DESCRIPTION: &'static str;
  /// A formatted message that includes specific parameters
  #[must_use]
  fn message(&self) -> String { self.description().to_string() }
  /// Code positions relevant to this error. If you don't implement this, you
  /// must implement [ProjectError::one_position]
  #[must_use]
  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> + '_ {
    box_once(ErrorPosition { origin: self.one_position(), message: None })
  }
  /// Short way to provide a single origin. If you don't implement this, you
  /// must implement [ProjectError::positions]
  #[must_use]
  fn one_position(&self) -> CodeOrigin { unimplemented!() }
  /// Convert the error into an `Arc<dyn DynProjectError>` to be able to
  /// handle various errors together
  #[must_use]
  fn pack(self) -> ProjectErrorObj { Arc::new(self) }
}

/// Object-safe version of [ProjectError]. Implement that instead of this.
pub trait DynProjectError: Send + Sync {
  /// Access type information about this error
  #[must_use]
  fn as_any_ref(&self) -> &dyn Any;
  /// Pack the error into a trait object, or leave it as-is if it's already a
  /// trait object
  #[must_use]
  fn into_packed(self: Arc<Self>) -> ProjectErrorObj;
  /// A general description of this type of error
  #[must_use]
  fn description(&self) -> &str;
  /// A formatted message that includes specific parameters
  #[must_use]
  fn message(&self) -> String { self.description().to_string() }
  /// Code positions relevant to this error.
  #[must_use]
  fn positions(&self) -> BoxedIter<'_, ErrorPosition>;
}

impl<T> DynProjectError for T
where T: ProjectError
{
  fn as_any_ref(&self) -> &dyn Any { self }
  fn into_packed(self: Arc<Self>) -> ProjectErrorObj { self }
  fn description(&self) -> &str { T::DESCRIPTION }
  fn message(&self) -> String { ProjectError::message(self) }
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    Box::new(ProjectError::positions(self).into_iter())
  }
}

impl DynProjectError for ProjectErrorObj {
  fn as_any_ref(&self) -> &dyn Any { (**self).as_any_ref() }
  fn description(&self) -> &str { (**self).description() }
  fn into_packed(self: Arc<Self>) -> ProjectErrorObj { (*self).clone() }
  fn message(&self) -> String { (**self).message() }
  fn positions(&self) -> BoxedIter<'_, ErrorPosition> { (**self).positions() }
}

impl fmt::Display for dyn DynProjectError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let description = self.description();
    let message = self.message();
    let positions = self.positions().collect::<Vec<_>>();
    writeln!(f, "Project error: {description}\n{message}")?;
    if positions.is_empty() {
      writeln!(f, "No origins specified")?;
    } else {
      for ErrorPosition { origin, message } in positions {
        match message {
          None => writeln!(f, "@{origin}"),
          Some(msg) => writeln!(f, "@{origin}: {msg}"),
        }?
      }
    }
    Ok(())
  }
}

impl fmt::Debug for dyn DynProjectError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self}") }
}

/// Type-erased [ProjectError] implementor through the [DynProjectError]
/// object-trait
pub type ProjectErrorObj = Arc<dyn DynProjectError>;
/// Alias for a result with an error of [ProjectErrorObj]. This is the type of
/// result most commonly returned by pre-runtime operations.
pub type ProjectResult<T> = Result<T, ProjectErrorObj>;

/// A trait for error types that are only missing an origin. Do not depend on
/// this trait, refer to [DynErrorSansOrigin] instead.
pub trait ErrorSansOrigin: Clone + Sized + Send + Sync + 'static {
  /// General description of the error condition
  const DESCRIPTION: &'static str;
  /// Specific description of the error including code fragments or concrete
  /// data if possible
  fn message(&self) -> String { Self::DESCRIPTION.to_string() }
  /// Convert the error to a type-erased structure for handling on shared
  /// channels
  fn pack(self) -> ErrorSansOriginObj { Box::new(self) }
  /// A shortcut to streamline switching code between [ErrorSansOriginObj] and
  /// concrete types
  fn bundle(self, origin: &CodeOrigin) -> ProjectErrorObj { self.pack().bundle(origin) }
}

/// Object-safe equivalent to [ErrorSansOrigin]. Implement that one instead of
/// this. Typically found as [ErrorSansOriginObj]
pub trait DynErrorSansOrigin: Any + Send + Sync + DynClone {
  /// Allow to downcast the base object to distinguish between various errors.
  /// The main intended purpose is to trigger a fallback when [CodeNotFound] is
  /// encountered, but the possibilities are not limited to that.
  fn as_any_ref(&self) -> &dyn Any;
  /// Regularize the type
  fn into_packed(self: Box<Self>) -> ErrorSansOriginObj;
  /// Generic description of the error condition
  fn description(&self) -> &str;
  /// Specific description of this particular error
  fn message(&self) -> String;
  /// Add an origin
  fn bundle(self: Box<Self>, origin: &CodeOrigin) -> ProjectErrorObj;
}

/// Type-erased [ErrorSansOrigin] implementor through the object-trait
/// [DynErrorSansOrigin]. This can be turned into a [ProjectErrorObj] with
/// [ErrorSansOriginObj::bundle].
pub type ErrorSansOriginObj = Box<dyn DynErrorSansOrigin>;
/// A generic project result without origin
pub type ResultSansOrigin<T> = Result<T, ErrorSansOriginObj>;

impl<T: ErrorSansOrigin + 'static> DynErrorSansOrigin for T {
  fn description(&self) -> &str { Self::DESCRIPTION }
  fn message(&self) -> String { (*self).message() }
  fn as_any_ref(&self) -> &dyn Any { self }
  fn into_packed(self: Box<Self>) -> ErrorSansOriginObj { (*self).pack() }
  fn bundle(self: Box<Self>, origin: &CodeOrigin) -> ProjectErrorObj {
    Arc::new(OriginBundle(origin.clone(), *self))
  }
}
impl Clone for ErrorSansOriginObj {
  fn clone(&self) -> Self { clone_box(&**self) }
}
impl DynErrorSansOrigin for ErrorSansOriginObj {
  fn description(&self) -> &str { (**self).description() }
  fn message(&self) -> String { (**self).message() }
  fn as_any_ref(&self) -> &dyn Any { (**self).as_any_ref() }
  fn into_packed(self: Box<Self>) -> ErrorSansOriginObj { *self }
  fn bundle(self: Box<Self>, origin: &CodeOrigin) -> ProjectErrorObj { (*self).bundle(origin) }
}
impl fmt::Display for ErrorSansOriginObj {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    writeln!(f, "{}\nOrigin missing from error", self.message())
  }
}
impl fmt::Debug for ErrorSansOriginObj {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self}") }
}

struct OriginBundle<T: ErrorSansOrigin>(CodeOrigin, T);
impl<T: ErrorSansOrigin> DynProjectError for OriginBundle<T> {
  fn as_any_ref(&self) -> &dyn Any { self.1.as_any_ref() }
  fn into_packed(self: Arc<Self>) -> ProjectErrorObj { self }
  fn description(&self) -> &str { self.1.description() }
  fn message(&self) -> String { self.1.message() }
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    box_once(ErrorPosition { origin: self.0.clone(), message: None })
  }
}

/// A collection for tracking fatal errors without halting. Participating
/// functions return [ProjectResult] even if they only ever construct [Ok]. When
/// they call other participating functions, instead of directly forwarding
/// errors with `?` they should prefer constructing a fallback value with
/// [Reporter::fallback]. If any error is added to a [Reporter] in a function,
/// the return value is valid but its meaning need not be related in any way to
/// the inputs.
///
/// Returning [Err] from a function that accepts `&mut Reporter` indicates not
/// that there was a fatal error but that it wasn't possible to construct a
/// fallback, so if it can, the caller should construct one.
pub struct Reporter(RefCell<Vec<ProjectErrorObj>>);
impl Reporter {
  /// Create a new error reporter
  pub fn new() -> Self { Self(RefCell::new(Vec::new())) }
  /// Returns true if any errors were regorded. If this ever returns true, it
  /// will always return true in the future.
  pub fn failing(&self) -> bool { !self.0.borrow().is_empty() }
  /// Report a fatal error
  pub fn report(&self, error: ProjectErrorObj) { self.0.borrow_mut().push(error) }
  /// Catch a fatal error, report it, and substitute the value
  pub fn fallback<T>(&self, res: ProjectResult<T>, cb: impl FnOnce(ProjectErrorObj) -> T) -> T {
    res.inspect_err(|e| self.report(e.clone())).unwrap_or_else(cb)
  }
  /// Panic if there were errors
  pub fn assert(&self) { self.unwrap(Ok(())) }
  /// Exit with code -1 if there were errors
  pub fn assert_exit(&self) { self.unwrap_exit(Ok(())) }
  /// Panic with descriptive messages if there were errors. If there were no
  /// errors, unwrap the result
  pub fn unwrap<T>(&self, res: ProjectResult<T>) -> T {
    if self.failing() {
      panic!("Errors were encountered: \n{}", self.0.borrow().iter().join("\n"));
    }
    res.unwrap()
  }
  /// Print errors and exit if any occurred.  If there were no errors, unwrap
  /// the result
  pub fn unwrap_exit<T>(&self, res: ProjectResult<T>) -> T {
    if self.failing() {
      eprintln!("Errors were encountered: \n{}", self.0.borrow().iter().join("\n"));
      process::exit(-1)
    }
    res.unwrap_or_else(|e| {
      eprintln!("{e}");
      process::exit(-1)
    })
  }
  /// Take the errors out of the reporter
  #[must_use]
  pub fn into_errors(self) -> Option<Vec<ProjectErrorObj>> {
    let v = self.0.into_inner();
    if v.is_empty() { None } else { Some(v) }
  }
  /// Raise an error if the reporter contains any errors
  pub fn bind(self) -> ProjectResult<()> {
    match self.into_errors() {
      None => Ok(()),
      Some(v) if v.len() == 1 => Err(v.into_iter().exactly_one().unwrap()),
      Some(v) => Err(MultiError(v).pack()),
    }
  }
}

impl Default for Reporter {
  fn default() -> Self { Self::new() }
}

struct MultiError(Vec<ProjectErrorObj>);
impl ProjectError for MultiError {
  const DESCRIPTION: &'static str = "Multiple errors occurred";
  fn message(&self) -> String { format!("{} errors occurred", self.0.len()) }
  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> + '_ {
    self.0.iter().flat_map(|e| {
      e.positions().map(|pos| {
        let emsg = e.message();
        let msg = if let Some(pmsg) = pos.message { format!("{emsg}: {pmsg}") } else { emsg };
        ErrorPosition { origin: pos.origin, message: Some(msg) }
      })
    })
  }
}
