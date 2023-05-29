use std::fmt::Debug;

use super::super::Numeric;
use crate::representations::interpreted::ExprInst;
use crate::{atomic_impl, atomic_redirect, externfn_impl};

/// Divides two numbers
///
/// Next state: [Divide1]

#[derive(Clone)]
pub struct Divide2;
externfn_impl!(Divide2, |_: &Self, x: ExprInst| Ok(Divide1 { x }));

/// Prev state: [Divide2]; Next state: [Divide0]
#[derive(Debug, Clone)]
pub struct Divide1 {
  x: ExprInst,
}
atomic_redirect!(Divide1, x);
atomic_impl!(Divide1);
externfn_impl!(Divide1, |this: &Self, x: ExprInst| {
  let a: Numeric = this.x.clone().try_into()?;
  Ok(Divide0 { a, x })
});

/// Prev state: [Divide1]
#[derive(Debug, Clone)]
pub struct Divide0 {
  a: Numeric,
  x: ExprInst,
}
atomic_redirect!(Divide0, x);
atomic_impl!(Divide0, |Self { a, x }: &Self, _| {
  let b: Numeric = x.clone().try_into()?;
  Ok((*a / b).into())
});