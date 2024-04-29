//! Error produced by numeric opperations

use std::fmt;

use crate::foreign::error::RTError;

/// Various errors produced by arithmetic operations
#[derive(Clone)]
pub enum ArithmeticError {
  /// Integer overflow
  Overflow,
  /// Float overflow
  Infinity,
  /// Division or modulo by zero
  DivByZero,
  /// Other, unexpected operation produced NaN
  NaN,
}

impl fmt::Display for ArithmeticError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::NaN => write!(f, "Operation resulted in NaN"),
      Self::Overflow => write!(f, "Integer overflow"),
      Self::Infinity => write!(f, "Operation resulted in Infinity"),
      Self::DivByZero => write!(f, "A division by zero was attempted"),
    }
  }
}

impl RTError for ArithmeticError {}
