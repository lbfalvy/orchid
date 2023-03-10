
use super::super::Numeric;

use std::fmt::Debug;
use std::hash::Hash;

use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::interpreted::Clause;

/// Multiply function
/// 
/// Next state: [Multiply1]

#[derive(Clone)]
pub struct Multiply2;
externfn_impl!(Multiply2, |_: &Self, c: Clause| {Ok(Multiply1{c})});

/// Partially applied Multiply function
/// 
/// Prev state: [Multiply2]; Next state: [Multiply0]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Multiply1{ c: Clause }
atomic_redirect!(Multiply1, c);
atomic_impl!(Multiply1);
externfn_impl!(Multiply1, |this: &Self, c: Clause| {
  let a: Numeric = this.c.clone().try_into()?;
  Ok(Multiply0{ a, c })
});

/// Fully applied Multiply function.
/// 
/// Prev state: [Multiply1]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Multiply0 { a: Numeric, c: Clause }
atomic_redirect!(Multiply0, c);
atomic_impl!(Multiply0, |Self{ a, c }: &Self| {
  let b: Numeric = c.clone().try_into()?;
  Ok((*a * b).into())
});