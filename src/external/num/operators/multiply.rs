use std::fmt::Debug;

use super::super::Numeric;
use crate::representations::interpreted::ExprInst;
use crate::{atomic_impl, atomic_redirect, externfn_impl};

/// Multiplies two numbers
///
/// Next state: [Multiply1]
#[derive(Clone)]
pub struct Multiply2;
externfn_impl!(Multiply2, |_: &Self, x: ExprInst| Ok(Multiply1 { x }));

/// Prev state: [Multiply2]; Next state: [Multiply0]
#[derive(Debug, Clone)]
pub struct Multiply1 {
  x: ExprInst,
}
atomic_redirect!(Multiply1, x);
atomic_impl!(Multiply1);
externfn_impl!(Multiply1, |this: &Self, x: ExprInst| {
  let a: Numeric = this.x.clone().try_into()?;
  Ok(Multiply0 { a, x })
});

/// Prev state: [Multiply1]
#[derive(Debug, Clone)]
pub struct Multiply0 {
  a: Numeric,
  x: ExprInst,
}
atomic_redirect!(Multiply0, x);
atomic_impl!(Multiply0, |Self { a, x }: &Self, _| {
  let b: Numeric = x.clone().try_into()?;
  Ok((*a * b).into())
});
