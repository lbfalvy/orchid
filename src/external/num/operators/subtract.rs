
use super::super::Numeric;

use std::fmt::Debug;
use std::hash::Hash;

use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::interpreted::{Clause};

/// Subtract function
/// 
/// Next state: [Subtract1]

#[derive(Clone)]
pub struct Subtract2;
externfn_impl!(Subtract2, |_: &Self, c: Clause| {Ok(Subtract1{c})});

/// Partially applied Subtract function
/// 
/// Prev state: [Subtract2]; Next state: [Subtract0]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Subtract1{ c: Clause }
atomic_redirect!(Subtract1, c);
atomic_impl!(Subtract1);
externfn_impl!(Subtract1, |this: &Self, c: Clause| {
  let a: Numeric = this.c.clone().try_into()?;
  Ok(Subtract0{ a, c })
});

/// Fully applied Subtract function.
/// 
/// Prev state: [Subtract1]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Subtract0 { a: Numeric, c: Clause }
atomic_redirect!(Subtract0, c);
atomic_impl!(Subtract0, |Self{ a, c }: &Self| {
  let b: Numeric = c.clone().try_into()?;
  Ok((*a - b).into())
});