
use super::super::Numeric;

use std::fmt::Debug;

use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::interpreted::ExprInst;

/// Add function
/// 
/// Next state: [Add1]

#[derive(Clone)]
pub struct Add2;
externfn_impl!(Add2, |_: &Self, x: ExprInst| Ok(Add1{x}));

/// Partially applied Add function
/// 
/// Prev state: [Add2]; Next state: [Add0]

#[derive(Debug, Clone)]
pub struct Add1{ x: ExprInst }
atomic_redirect!(Add1, x);
atomic_impl!(Add1);
externfn_impl!(Add1, |this: &Self, x: ExprInst| {
  let a: Numeric = this.x.clone().try_into()?;
  Ok(Add0{ a, x })
});

/// Fully applied Add function.
/// 
/// Prev state: [Add1]

#[derive(Debug, Clone)]
pub struct Add0 { a: Numeric, x: ExprInst }
atomic_redirect!(Add0, x);
atomic_impl!(Add0, |Self{ a, x }: &Self| {
  let b: Numeric = x.clone().try_into()?;
  Ok((*a + b).into())
});
