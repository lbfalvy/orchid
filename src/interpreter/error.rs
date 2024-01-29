use std::fmt::{self, Debug, Display};

use itertools::Itertools;

use super::nort::Expr;
use super::run::Interrupted;
use crate::foreign::error::{ExternError, ExternErrorObj};
use crate::location::CodeLocation;
use crate::name::Sym;

/// Print a stack trace
pub fn strace(stack: &[Expr]) -> String {
  stack.iter().rev().map(|x| format!("{x}\n    at {}", x.location)).join("\n")
}

/// Problems in the process of execution
#[derive(Debug, Clone)]
pub enum RunError {
  /// A Rust function encountered an error
  Extern(ExternErrorObj),
  /// Ran out of gas
  Interrupted(Interrupted),
}

impl<T: ExternError + 'static> From<T> for RunError {
  fn from(value: T) -> Self { Self::Extern(value.rc()) }
}

impl From<ExternErrorObj> for RunError {
  fn from(value: ExternErrorObj) -> Self { Self::Extern(value) }
}

impl Display for RunError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Interrupted(i) => {
        write!(f, "Ran out of gas:\n{}", strace(&i.stack))
      },
      Self::Extern(e) => write!(f, "Program fault: {e}"),
    }
  }
}

#[derive(Clone)]
pub(crate) struct StackOverflow {
  pub stack: Vec<Expr>,
}
impl ExternError for StackOverflow {}
impl Display for StackOverflow {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let limit = self.stack.len() - 2; // 1 for failed call, 1 for current
    write!(f, "Stack depth exceeded {limit}:\n{}", strace(&self.stack))
  }
}

#[derive(Clone)]
pub(crate) struct MissingSymbol {
  pub sym: Sym,
  pub loc: CodeLocation,
}
impl ExternError for MissingSymbol {}
impl Display for MissingSymbol {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}, called at {} is not loaded", self.sym, self.loc)
  }
}
