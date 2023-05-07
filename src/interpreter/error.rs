use std::fmt::Display;
use std::rc::Rc;

use crate::representations::interpreted::ExprInst;
use crate::foreign::ExternError;

/// Problems in the process of execution
#[derive(Clone)]
pub enum RuntimeError {
  Extern(Rc<dyn ExternError>),
  NonFunctionApplication(ExprInst),
}

impl From<Rc<dyn ExternError>> for RuntimeError {
  fn from(value: Rc<dyn ExternError>) -> Self {
    Self::Extern(value)
  }
}

impl Display for RuntimeError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Extern(e) => write!(f, "Error in external function: {e}"),
      Self::NonFunctionApplication(loc) => write!(f, "Primitive applied as function at {loc:?}")
    }
  }
}