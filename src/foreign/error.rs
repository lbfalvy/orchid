use std::error::Error;
use std::fmt::{Debug, Display};
use std::sync::Arc;

use dyn_clone::DynClone;

use crate::location::CodeLocation;

/// Errors produced by external code
pub trait ExternError: Display + Send + Sync + DynClone {
  /// Convert into trait object
  #[must_use]
  fn rc(self) -> Arc<dyn ExternError>
  where Self: 'static + Sized {
    Arc::new(self)
  }
}

impl Debug for dyn ExternError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "ExternError({self})")
  }
}

impl Error for dyn ExternError {}

/// An error produced by Rust code called form Orchid. The error is type-erased.
pub type ExternResult<T> = Result<T, Arc<dyn ExternError>>;

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
  pub fn fail<T>(
    location: CodeLocation,
    message: &'static str,
    details: String,
  ) -> ExternResult<T> {
    Err(Self::ext(location, message, details))
  }

  /// Construct and upcast to [ExternError]
  pub fn ext(
    location: CodeLocation,
    message: &'static str,
    details: String,
  ) -> Arc<dyn ExternError> {
    Self { location, message, details }.rc()
  }
}

impl Display for AssertionError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Error: expected {}", self.message)?;
    write!(f, " at {}", self.location)?;
    write!(f, " details: {}", self.details)
  }
}

impl ExternError for AssertionError {}
