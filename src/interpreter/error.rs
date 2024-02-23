//! Error produced by the interpreter.

use std::fmt;

use super::run::State;
use crate::foreign::error::{RTError, RTErrorObj};

/// Error produced by the interpreter. This could be because the code is faulty,
/// but equally because gas was being counted and it ran out.
#[derive(Debug)]
pub enum RunError<'a> {
  /// A Rust function encountered an error
  Extern(RTErrorObj),
  /// Ran out of gas
  Interrupted(State<'a>),
}

impl<'a, T: RTError + 'static> From<T> for RunError<'a> {
  fn from(value: T) -> Self { Self::Extern(value.pack()) }
}

impl<'a> From<RTErrorObj> for RunError<'a> {
  fn from(value: RTErrorObj) -> Self { Self::Extern(value) }
}

impl<'a> fmt::Display for RunError<'a> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Interrupted(i) => write!(f, "Ran out of gas:\n{i}"),
      Self::Extern(e) => write!(f, "Program fault: {e}"),
    }
  }
}
