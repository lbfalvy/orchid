use std::rc::Rc;

use crate::interner::InternedDisplay;
use crate::representations::location::Location;
use crate::utils::iter::box_once;
use crate::utils::BoxedIter;
use crate::Interner;

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
  fn message(&self, _i: &Interner) -> String {
    self.description().to_string()
  }
  /// Code positions relevant to this error. If you don't implement this, you
  /// must implement [ProjectError::one_position]
  fn positions(&self, i: &Interner) -> BoxedIter<ErrorPosition> {
    box_once(ErrorPosition { location: self.one_position(i), message: None })
  }
  /// Short way to provide a single location. If you don't implement this, you
  /// must implement [ProjectError::positions]
  fn one_position(&self, _i: &Interner) -> Location {
    unimplemented!()
  }
  /// Convert the error into an `Rc<dyn ProjectError>` to be able to
  /// handle various errors together
  fn rc(self) -> Rc<dyn ProjectError>
  where
    Self: Sized + 'static,
  {
    Rc::new(self)
  }
}

impl InternedDisplay for dyn ProjectError {
  fn fmt_i(
    &self,
    f: &mut std::fmt::Formatter<'_>,
    i: &Interner,
  ) -> std::fmt::Result {
    let description = self.description();
    let message = self.message(i);
    let positions = self.positions(i);
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

/// Alias for a result with an error of [Rc] of [ProjectError] trait object.
/// This is the type of result most commonly returned by pre-run operations.
pub type ProjectResult<T> = Result<T, Rc<dyn ProjectError>>;
