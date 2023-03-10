use std::fmt::Debug;
use std::hash::Hash;

use crate::external::assertion_error::AssertionError;
use crate::external::runtime_error::RuntimeError;
use crate::representations::{Literal, Primitive};
use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::interpreted::Clause;

/// CharAt function
/// 
/// Next state: [CharAt1]

#[derive(Clone)]
pub struct CharAt2;
externfn_impl!(CharAt2, |_: &Self, c: Clause| {Ok(CharAt1{c})});

/// Partially applied CharAt function
/// 
/// Prev state: [CharAt2]; Next state: [CharAt0]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct CharAt1{ c: Clause }
atomic_redirect!(CharAt1, c);
atomic_impl!(CharAt1);
externfn_impl!(CharAt1, |this: &Self, c: Clause| {
  let s = if let Ok(Literal::Str(s)) = this.c.clone().try_into() {s}
  else {AssertionError::fail(this.c.clone(), "a string")?};
  Ok(CharAt0{ s, c })
});

/// Fully applied CharAt function.
/// 
/// Prev state: [CharAt1]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct CharAt0 { s: String, c: Clause }
atomic_redirect!(CharAt0, c);
atomic_impl!(CharAt0, |Self{ s, c }: &Self| {
  let i = if let Ok(Literal::Uint(i)) = c.clone().try_into() {i}
  else {AssertionError::fail(c.clone(), "an uint")?};
  if let Some(c) = s.chars().nth(i as usize) {
    Ok(Clause::P(Primitive::Literal(Literal::Char(c))))
  } else {
    RuntimeError::fail("Character index out of bounds".to_string(), "indexing string")?
  }
});
