use std::fmt::Debug;

use crate::external::litconv::{with_str, with_uint};
use crate::external::runtime_error::RuntimeError;
use crate::representations::{Literal, Primitive};
use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::interpreted::{Clause, ExprInst};

/// CharAt function
/// 
/// Next state: [CharAt1]

#[derive(Clone)]
pub struct CharAt2;
externfn_impl!(CharAt2, |_: &Self, x: ExprInst| Ok(CharAt1{x}));

/// Partially applied CharAt function
/// 
/// Prev state: [CharAt2]; Next state: [CharAt0]

#[derive(Debug, Clone)]
pub struct CharAt1{ x: ExprInst }
atomic_redirect!(CharAt1, x);
atomic_impl!(CharAt1);
externfn_impl!(CharAt1, |this: &Self, x: ExprInst| {
  with_str(&this.x, |s| Ok(CharAt0{ s: s.clone(), x }))
});

/// Fully applied CharAt function.
/// 
/// Prev state: [CharAt1]

#[derive(Debug, Clone)]
pub struct CharAt0 { s: String, x: ExprInst }
atomic_redirect!(CharAt0, x);
atomic_impl!(CharAt0, |Self{ s, x }: &Self| {
  with_uint(x, |i| if let Some(c) = s.chars().nth(i as usize) {
    Ok(Clause::P(Primitive::Literal(Literal::Char(c))))
  } else {
    RuntimeError::fail("Character index out of bounds".to_string(), "indexing string")?
  })
});
