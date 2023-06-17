use std::fmt::Display;

use crate::foreign::ExternError;

/// Various errors produced by arithmetic operations
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

impl Display for ArithmeticError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::NaN => write!(f, "Operation resulted in NaN"),
      Self::Overflow => write!(f, "Integer overflow"),
      Self::Infinity => write!(f, "Operation resulted in Infinity"),
      Self::DivByZero => write!(f, "A division by zero was attempted"),
    }
  }
}

impl ExternError for ArithmeticError {}
