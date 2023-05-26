use std::fmt::Debug;

use super::super::litconv::{with_str, with_uint};
use super::super::runtime_error::RuntimeError;
use crate::representations::interpreted::{Clause, ExprInst};
use crate::representations::{Literal, Primitive};
use crate::{atomic_impl, atomic_redirect, externfn_impl};

/// Takes an uint and a string, finds the char in a string at a 0-based index
///
/// Next state: [CharAt1]
#[derive(Clone)]
pub struct CharAt2;
externfn_impl!(CharAt2, |_: &Self, x: ExprInst| Ok(CharAt1 { x }));

/// Prev state: [CharAt2]; Next state: [CharAt0]
#[derive(Debug, Clone)]
pub struct CharAt1 {
  x: ExprInst,
}
atomic_redirect!(CharAt1, x);
atomic_impl!(CharAt1);
externfn_impl!(CharAt1, |this: &Self, x: ExprInst| {
  with_str(&this.x, |s| Ok(CharAt0 { s: s.clone(), x }))
});

/// Prev state: [CharAt1]
#[derive(Debug, Clone)]
pub struct CharAt0 {
  s: String,
  x: ExprInst,
}
atomic_redirect!(CharAt0, x);
atomic_impl!(CharAt0, |Self { s, x }: &Self, _| {
  with_uint(x, |i| {
    if let Some(c) = s.chars().nth(i as usize) {
      Ok(Clause::P(Primitive::Literal(Literal::Char(c))))
    } else {
      RuntimeError::fail(
        "Character index out of bounds".to_string(),
        "indexing string",
      )?
    }
  })
});
