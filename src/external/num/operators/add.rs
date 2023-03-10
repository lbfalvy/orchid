
use super::super::Numeric;

use std::fmt::Debug;
use std::hash::Hash;

use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::interpreted::Clause;

/// Add function
/// 
/// Next state: [Add1]

#[derive(Clone)]
pub struct Add2;
externfn_impl!(Add2, |_: &Self, c: Clause| {Ok(Add1{c})});

/// Partially applied Add function
/// 
/// Prev state: [Add2]; Next state: [Add0]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Add1{ c: Clause }
atomic_redirect!(Add1, c);
atomic_impl!(Add1);
externfn_impl!(Add1, |this: &Self, c: Clause| {
  let a: Numeric = this.c.clone().try_into()?;
  Ok(Add0{ a, c })
});

/// Fully applied Add function.
/// 
/// Prev state: [Add1]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Add0 { a: Numeric, c: Clause }
atomic_redirect!(Add0, c);
atomic_impl!(Add0, |Self{ a, c }: &Self| {
  let b: Numeric = c.clone().try_into()?;
  Ok((*a + b).into())
});
