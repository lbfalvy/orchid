use std::fmt::Debug;

use super::super::Numeric;
use crate::representations::interpreted::ExprInst;
use crate::{atomic_impl, atomic_redirect, externfn_impl};

/// Takes the modulus of two numbers.
///
/// Next state: [Remainder1]
#[derive(Clone)]
pub struct Remainder2;
externfn_impl!(Remainder2, |_: &Self, x: ExprInst| Ok(Remainder1 { x }));

/// Prev state: [Remainder2]; Next state: [Remainder0]
#[derive(Debug, Clone)]
pub struct Remainder1 {
  x: ExprInst,
}
atomic_redirect!(Remainder1, x);
atomic_impl!(Remainder1);
externfn_impl!(Remainder1, |this: &Self, x: ExprInst| {
  let a: Numeric = this.x.clone().try_into()?;
  Ok(Remainder0 { a, x })
});

/// Prev state: [Remainder1]
#[derive(Debug, Clone)]
pub struct Remainder0 {
  a: Numeric,
  x: ExprInst,
}
atomic_redirect!(Remainder0, x);
atomic_impl!(Remainder0, |Self { a, x }: &Self, _| {
  let b: Numeric = x.clone().try_into()?;
  Ok((*a % b).into())
});