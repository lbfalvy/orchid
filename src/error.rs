//! Abstractions for handling various code-related errors under a common trait
//! object.

use core::fmt;
use std::any::Any;
use std::fmt::{Debug, Display};
use std::sync::Arc;

use dyn_clone::{clone_box, DynClone};

use crate::location::CodeLocation;
use crate::utils::boxed_iter::{box_once, BoxedIter};
#[allow(unused)] // for doc
use crate::virt_fs::CodeNotFound;

/// A point of interest in resolving the error, such as the point where
/// processing got stuck, a command that is likely to be incorrect
pub struct ErrorPosition {
  /// The suspected location
  pub location: CodeLocation,
  /// Any information about the role of this location
  pub message: Option<String>,
}
impl From<CodeLocation> for ErrorPosition {
  fn from(location: CodeLocation) -> Self { Self { location, message: None } }
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
  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> {
    box_once(ErrorPosition { location: self.one_position(), message: None })
  }
  /// Short way to provide a single location. If you don't implement this, you
  /// must implement [ProjectError::positions]
  #[must_use]
  fn one_position(&self) -> CodeLocation { unimplemented!() }
  /// Convert the error into an `Arc<dyn DynProjectError>` to be able to
  /// handle various errors together
  #[must_use]
  fn pack(self) -> ProjectErrorObj { Arc::new(self) }
}

/// Object-safe version of [ProjectError]. Implement that instead of this.
pub trait DynProjectError: Send + Sync {
  /// Access type information about this error
  #[must_use]
  fn as_any(&self) -> &dyn Any;
  /// A general description of this type of error
  #[must_use]
  fn description(&self) -> &str;
  /// A formatted message that includes specific parameters
  #[must_use]
  fn message(&self) -> String { self.description().to_string() }
  /// Code positions relevant to this error.
  #[must_use]
  fn positions(&self) -> BoxedIter<ErrorPosition>;
}

impl<T> DynProjectError for T
where T: ProjectError
{
  fn as_any(&self) -> &dyn Any { self }
  fn description(&self) -> &str { T::DESCRIPTION }
  fn message(&self) -> String { ProjectError::message(self) }
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    Box::new(ProjectError::positions(self).into_iter())
  }
}

impl Display for dyn DynProjectError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let description = self.description();
    let message = self.message();
    let positions = self.positions().collect::<Vec<_>>();
    writeln!(f, "Project error: {description}\n{message}")?;
    if positions.is_empty() {
      writeln!(f, "No locations specified")?;
    } else {
      for ErrorPosition { location, message } in positions {
        match message {
          None => writeln!(f, "@{location}"),
          Some(msg) => writeln!(f, "@{location}: {msg}"),
        }?
      }
    }
    Ok(())
  }
}

impl Debug for dyn DynProjectError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{self}")
  }
}

/// Type-erased [ProjectError] implementor through the [DynProjectError]
/// object-trait
pub type ProjectErrorObj = Arc<dyn DynProjectError>;
/// Alias for a result with an error of [Rc] of [ProjectError] trait object.
/// This is the type of result most commonly returned by pre-run operations.
pub type ProjectResult<T> = Result<T, ProjectErrorObj>;

/// A trait for error types that are only missing a location. Do not depend on
/// this trait, refer to [DynErrorSansLocation] instead.
pub trait ErrorSansLocation: Clone + Sized + Send + Sync + 'static {
  /// General description of the error condition
  const DESCRIPTION: &'static str;
  /// Specific description of the error including code fragments or concrete
  /// data if possible
  fn message(&self) -> String { Self::DESCRIPTION.to_string() }
  /// Convert the error to a type-erased structure for handling on shared
  /// channels
  fn pack(self) -> ErrorSansLocationObj { Box::new(self) }
}

/// Object-safe equivalent to [ErrorSansLocation]. Implement that one instead of
/// this. Typically found as [ErrorSansLocationObj]
pub trait DynErrorSansLocation: Any + Send + Sync + DynClone {
  /// Allow to downcast the base object to distinguish between various errors.
  /// The main intended purpose is to trigger a fallback when [CodeNotFound] is
  /// encountered, but the possibilities are not limited to that.
  fn as_any_ref(&self) -> &dyn Any;
  /// Generic description of the error condition
  fn description(&self) -> &str;
  /// Specific description of this particular error
  fn message(&self) -> String;
}

/// Type-erased [ErrorSansLocation] implementor through the object-trait
/// [DynErrorSansLocation]. This can be turned into a [ProjectErrorObj] with
/// [bundle_location].
pub type ErrorSansLocationObj = Box<dyn DynErrorSansLocation>;
/// A generic project result without location
pub type ResultSansLocation<T> = Result<T, ErrorSansLocationObj>;

impl<T: ErrorSansLocation + 'static> DynErrorSansLocation for T {
  fn description(&self) -> &str { Self::DESCRIPTION }
  fn message(&self) -> String { self.message() }
  fn as_any_ref(&self) -> &dyn Any { self }
}
impl Clone for ErrorSansLocationObj {
  fn clone(&self) -> Self { clone_box(&**self) }
}
impl DynErrorSansLocation for ErrorSansLocationObj {
  fn description(&self) -> &str { (**self).description() }
  fn message(&self) -> String { (**self).message() }
  fn as_any_ref(&self) -> &dyn Any { (**self).as_any_ref() }
}
impl Display for ErrorSansLocationObj {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    writeln!(f, "{}\nLocation missing from error", self.message())
  }
}
impl Debug for ErrorSansLocationObj {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{self}")
  }
}

struct LocationBundle(CodeLocation, Box<dyn DynErrorSansLocation>);
impl DynProjectError for LocationBundle {
  fn as_any(&self) -> &dyn Any { self.1.as_any_ref() }
  fn description(&self) -> &str { self.1.description() }
  fn message(&self) -> String { self.1.message() }
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    box_once(ErrorPosition { location: self.0.clone(), message: None })
  }
}

/// Add a location to an [ErrorSansLocation]
pub fn bundle_location(
  location: &CodeLocation,
  details: &dyn DynErrorSansLocation,
) -> ProjectErrorObj {
  Arc::new(LocationBundle(location.clone(), clone_box(details)))
}
