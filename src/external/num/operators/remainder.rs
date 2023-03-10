
use super::super::Numeric;

use std::fmt::Debug;
use std::hash::Hash;

use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::interpreted::Clause;

/// Remainder function
/// 
/// Next state: [Remainder1]

#[derive(Clone)]
pub struct Remainder2;
externfn_impl!(Remainder2, |_: &Self, c: Clause| {Ok(Remainder1{c})});

/// Partially applied Remainder function
/// 
/// Prev state: [Remainder2]; Next state: [Remainder0]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Remainder1{ c: Clause }
atomic_redirect!(Remainder1, c);
atomic_impl!(Remainder1);
externfn_impl!(Remainder1, |this: &Self, c: Clause| {
  let a: Numeric = this.c.clone().try_into()?;
  Ok(Remainder0{ a, c })
});

/// Fully applied Remainder function.
/// 
/// Prev state: [Remainder1]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Remainder0 { a: Numeric, c: Clause }
atomic_redirect!(Remainder0, c);
atomic_impl!(Remainder0, |Self{ a, c }: &Self| {
  let b: Numeric = c.clone().try_into()?;
  Ok((*a % b).into())
});