use std::fmt::Display;
use std::rc::Rc;

use crate::foreign::ExternError;
use crate::representations::interpreted::ExprInst;
use crate::{Location, Sym};

/// Problems in the process of execution
#[derive(Clone, Debug)]
pub enum RuntimeError {
  /// A Rust function encountered an error
  Extern(Rc<dyn ExternError>),
  /// Primitive applied as function
  NonFunctionApplication(ExprInst),
  /// Symbol not in context
  MissingSymbol(Sym, Location),
}

impl From<Rc<dyn ExternError>> for RuntimeError {
  fn from(value: Rc<dyn ExternError>) -> Self { Self::Extern(value) }
}

impl Display for RuntimeError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Extern(e) => write!(f, "Error in external function: {e}"),
      Self::NonFunctionApplication(expr) => {
        write!(f, "Primitive applied as function at {}", expr.expr().location)
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
