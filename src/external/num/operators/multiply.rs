
use super::super::Numeric;

use std::fmt::Debug;

use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::interpreted::ExprInst;

/// Multiply function
/// 
/// Next state: [Multiply1]

#[derive(Clone)]
pub struct Multiply2;
externfn_impl!(Multiply2, |_: &Self, x: ExprInst| Ok(Multiply1{x}));

/// Partially applied Multiply function
/// 
/// Prev state: [Multiply2]; Next state: [Multiply0]

#[derive(Debug, Clone)]
pub struct Multiply1{ x: ExprInst }
atomic_redirect!(Multiply1, x);
atomic_impl!(Multiply1);
externfn_impl!(Multiply1, |this: &Self, x: ExprInst| {
  let a: Numeric = this.x.clone().try_into()?;
  Ok(Multiply0{ a, x })
});

/// Fully applied Multiply function.
/// 
/// Prev state: [Multiply1]

#[derive(Debug, Clone)]
pub struct Multiply0 { a: Numeric, x: ExprInst }
atomic_redirect!(Multiply0, x);
atomic_impl!(Multiply0, |Self{ a, x }: &Self, _| {
  let b: Numeric = x.clone().try_into()?;
  Ok((*a * b).into())
});