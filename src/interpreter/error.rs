use std::fmt::{Debug, Display};
use std::sync::Arc;

use crate::foreign::error::ExternError;
use crate::location::CodeLocation;
use crate::name::Sym;

use super::run::Interrupted;

/// Problems in the process of execution
#[derive(Debug, Clone)]
pub enum RunError {
  /// A Rust function encountered an error
  Extern(Arc<dyn ExternError>),
  /// Symbol not in context
  MissingSymbol(Sym, CodeLocation),
  /// Ran out of gas
  Interrupted(Interrupted)
}

impl From<Arc<dyn ExternError>> for RunError {
  fn from(value: Arc<dyn ExternError>) -> Self { Self::Extern(value) }
}

impl Display for RunError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Extern(e) => write!(f, "Error in external function: {e}"),
      Self::MissingSymbol(sym, loc) => {
        write!(f, "{sym}, called at {loc} is not loaded")
      },
    }
  }
}
