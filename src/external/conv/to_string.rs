
use std::fmt::Debug;
use std::hash::Hash;

use crate::external::assertion_error::AssertionError;
use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::{Primitive, Literal};
use crate::representations::interpreted::Clause;

/// ToString a clause
/// 
/// Next state: [ToString0]

#[derive(Clone)]
pub struct ToString1;
externfn_impl!(ToString1, |_: &Self, c: Clause| {Ok(ToString0{c})});

/// Applied ToString function
/// 
/// Prev state: [ToString1]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct ToString0{ c: Clause }
atomic_redirect!(ToString0, c);
atomic_impl!(ToString0, |Self{ c }: &Self| {
  let literal: &Literal = c.try_into()
    .map_err(|_| AssertionError::ext(c.clone(), "a literal value"))?;
  let string = match literal {
    Literal::Char(c) => c.to_string(),
    Literal::Uint(i) => i.to_string(),
    Literal::Num(n) => n.to_string(),
    Literal::Str(s) => s.clone()
  };
  Ok(Clause::P(Primitive::Literal(Literal::Str(string))))
});
