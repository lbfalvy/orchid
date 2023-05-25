use std::fmt::Debug;

use super::super::Numeric;
use crate::representations::interpreted::ExprInst;
use crate::{atomic_impl, atomic_redirect, externfn_impl};

/// Subtracts two numbers
///
/// Next state: [Subtract1]
#[derive(Clone)]
pub struct Subtract2;
externfn_impl!(Subtract2, |_: &Self, x: ExprInst| Ok(Subtract1 { x }));

/// Prev state: [Subtract2]; Next state: [Subtract0]
#[derive(Debug, Clone)]
pub struct Subtract1 {
  x: ExprInst,
}
atomic_redirect!(Subtract1, x);
atomic_impl!(Subtract1);
externfn_impl!(Subtract1, |this: &Self, x: ExprInst| {
  let a: Numeric = this.x.clone().try_into()?;
  Ok(Subtract0 { a, x })
});

/// Prev state: [Subtract1]
#[derive(Debug, Clone)]
pub struct Subtract0 {
  a: Numeric,
  x: ExprInst,
}
atomic_redirect!(Subtract0, x);
atomic_impl!(Subtract0, |Self { a, x }: &Self, _| {
  let b: Numeric = x.clone().try_into()?;
  Ok((*a - b).into())
});
