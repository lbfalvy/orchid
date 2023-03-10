
use super::super::Numeric;

use std::fmt::Debug;
use std::hash::Hash;

use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::interpreted::Clause;

/// Divide function
/// 
/// Next state: [Divide1]

#[derive(Clone)]
pub struct Divide2;
externfn_impl!(Divide2, |_: &Self, c: Clause| {Ok(Divide1{c})});

/// Partially applied Divide function
/// 
/// Prev state: [Divide2]; Next state: [Divide0]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Divide1{ c: Clause }
atomic_redirect!(Divide1, c);
atomic_impl!(Divide1);
externfn_impl!(Divide1, |this: &Self, c: Clause| {
  let a: Numeric = this.c.clone().try_into()?;
  Ok(Divide0{ a, c })
});

/// Fully applied Divide function.
/// 
/// Prev state: [Divide1]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Divide0 { a: Numeric, c: Clause }
atomic_redirect!(Divide0, c);
atomic_impl!(Divide0, |Self{ a, c }: &Self| {
  let b: Numeric = c.clone().try_into()?;
  Ok((*a / b).into())
});