//! Errors produced by the interpreter

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use dyn_clone::DynClone;

use crate::error::ProjectErrorObj;
use crate::location::CodeLocation;

/// Errors produced by external code when runtime-enforced assertions are
/// violated.
pub trait RTError: fmt::Display + Send + Sync + DynClone {
  /// Convert into trait object
  #[must_use]
  fn pack(self) -> RTErrorObj
  where Self: 'static + Sized {
    Arc::new(self)
  }
}

impl fmt::Debug for dyn RTError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "ExternError({self})") }
}

impl Error for dyn RTError {}

impl RTError for ProjectErrorObj {}

/// An error produced by Rust code called form Orchid. The error is type-erased.
pub type RTErrorObj = Arc<dyn RTError>;

/// A result produced by Rust code called from Orchid.
pub type RTResult<T> = Result<T, RTErrorObj>;

/// Some expectation (usually about the argument types of a function) did not
/// hold.
#[derive(Clone)]
pub struct AssertionError {
  location: CodeLocation,
  message: &'static str,
  details: String,
}

impl AssertionError {
  /// Construct, upcast and wrap in a Result that never succeeds for easy
  /// short-circuiting
  pub fn fail<T>(location: CodeLocation, message: &'static str, details: String) -> RTResult<T> {
    Err(Self::ext(location, message, details))
  }

  /// Construct and upcast to [RTErrorObj]
  pub fn ext(location: CodeLocation, message: &'static str, details: String) -> RTErrorObj {
    Self { location, message, details }.pack()
  }
}

impl fmt::Display for AssertionError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Error: expected {}", self.message)?;
    write!(f, " at {}", self.location)?;
    write!(f, " details: {}", self.details)
  }
}

impl RTError for AssertionError {}
