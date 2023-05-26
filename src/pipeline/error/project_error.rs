use std::fmt::{Debug, Display};
use std::rc::Rc;

use crate::representations::location::Location;
use crate::utils::BoxedIter;

/// A point of interest in resolving the error, such as the point where
/// processing got stuck, a command that is likely to be incorrect
pub struct ErrorPosition {
  pub location: Location,
  pub message: Option<String>,
}

impl ErrorPosition {
  /// An error position referring to an entire file with no comment
  pub fn just_file(file: Vec<String>) -> Self {
    Self { message: None, location: Location::File(Rc::new(file)) }
  }
}

/// Errors addressed to the developer which are to be resolved with
/// code changes
pub trait ProjectError: Debug {
  /// A general description of this type of error
  fn description(&self) -> &str;
  /// A formatted message that includes specific parameters
  fn message(&self) -> String {
    String::new()
  }
  /// Code positions relevant to this error
  fn positions(&self) -> BoxedIter<ErrorPosition>;
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
    write!(f, "Problem with the project: {description}; {message}")?;
    for ErrorPosition { location, message } in positions {
      write!(
        f,
        "@{location}: {}",
        message.unwrap_or("location of interest".to_string())
      )?
    }
    Ok(())
  }
}
