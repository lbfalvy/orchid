use std::fmt::Debug;

use super::super::litconv::with_str;
use crate::representations::interpreted::{Clause, ExprInst};
use crate::representations::{Literal, Primitive};
use crate::{atomic_impl, atomic_redirect, externfn_impl};

/// Concatenates two strings
///
/// Next state: [Concatenate1]
#[derive(Clone)]
pub struct Concatenate2;
externfn_impl!(Concatenate2, |_: &Self, c: ExprInst| Ok(Concatenate1 { c }));

/// Prev state: [Concatenate2]; Next state: [Concatenate0]
#[derive(Debug, Clone)]
pub struct Concatenate1 {
  c: ExprInst,
}
atomic_redirect!(Concatenate1, c);
atomic_impl!(Concatenate1);
externfn_impl!(Concatenate1, |this: &Self, c: ExprInst| {
  with_str(&this.c, |a| Ok(Concatenate0 { a: a.clone(), c }))
});

/// Prev state: [Concatenate1]
#[derive(Debug, Clone)]
pub struct Concatenate0 {
  a: String,
  c: ExprInst,
}
atomic_redirect!(Concatenate0, c);
atomic_impl!(Concatenate0, |Self { a, c }: &Self, _| {
  with_str(c, |b| {
    Ok(Clause::P(Primitive::Literal(Literal::Str(a.to_owned() + b))))
  })
});
