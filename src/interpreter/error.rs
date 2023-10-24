use std::fmt::{Debug, Display};
use std::sync::Arc;

use crate::foreign::ExternError;
use crate::{Location, Sym};

/// Problems in the process of execution
#[derive(Debug, Clone)]
pub enum RuntimeError {
  /// A Rust function encountered an error
  Extern(Arc<dyn ExternError>),
  /// Primitive applied as function
  NonFunctionApplication(Location),
  /// Symbol not in context
  MissingSymbol(Sym, Location),
}

impl From<Arc<dyn ExternError>> for RuntimeError {
  fn from(value: Arc<dyn ExternError>) -> Self { Self::Extern(value) }
}

impl Display for RuntimeError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Extern(e) => write!(f, "Error in external function: {e}"),
      Self::NonFunctionApplication(location) => {
        write!(f, "Primitive applied as function at {}", location)
      },
      Self::MissingSymbol(sym, loc) => {
        write!(
          f,
          "{}, called at {loc} is not loaded",
          sym.extern_vec().join("::")
        )
      },
    }
  }
}
