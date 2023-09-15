use std::fmt::{Debug, Display};
use std::rc::Rc;

use crate::representations::location::Location;
use crate::utils::boxed_iter::box_once;
use crate::utils::BoxedIter;

/// A point of interest in resolving the error, such as the point where
/// processing got stuck, a command that is likely to be incorrect
pub struct ErrorPosition {
  /// The suspected location
  pub location: Location,
  /// Any information about the role of this location
  pub message: Option<String>,
}

/// Errors addressed to the developer which are to be resolved with
/// code changes
pub trait ProjectError {
  /// A general description of this type of error
  fn description(&self) -> &str;
  /// A formatted message that includes specific parameters
  fn message(&self) -> String { self.description().to_string() }
  /// Code positions relevant to this error. If you don't implement this, you
  /// must implement [ProjectError::one_position]
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    box_once(ErrorPosition { location: self.one_position(), message: None })
  }
  /// Short way to provide a single location. If you don't implement this, you
  /// must implement [ProjectError::positions]
  fn one_position(&self) -> Location { unimplemented!() }
  /// Convert the error into an `Rc<dyn ProjectError>` to be able to
  /// handle various errors together
  fn rc(self) -> Rc<dyn ProjectError>
  where
    Self: Sized + 'static,
  {
    Rc::new(self)
  }
}

impl Display for dyn ProjectError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let description = self.description();
    let message = self.message();
    let positions = self.positions();
    writeln!(f, "Project error: {description}\n{message}")?;
    for ErrorPosition { location, message } in positions {
      writeln!(
        f,
        "@{location}: {}",
        message.unwrap_or("location of interest".to_string())
      )?
    }
    Ok(())
  }
}

impl Debug for dyn ProjectError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{self}")
  }
}

/// Alias for a result with an error of [Rc] of [ProjectError] trait object.
/// This is the type of result most commonly returned by pre-run operations.
pub type ProjectResult<T> = Result<T, Rc<dyn ProjectError>>;
