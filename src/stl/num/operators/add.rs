use std::fmt::Debug;

use super::super::Numeric;
use crate::representations::interpreted::ExprInst;
use crate::{atomic_impl, atomic_redirect, externfn_impl};

#[derive(Clone)]
pub struct Add2;
externfn_impl!(Add2, |_: &Self, x: ExprInst| Ok(Add1 { x }));

#[derive(Debug, Clone)]
pub struct Add1 {
  x: ExprInst,
}
atomic_redirect!(Add1, x);
atomic_impl!(Add1);
externfn_impl!(Add1, |this: &Self, x: ExprInst| {
  let a: Numeric = this.x.clone().try_into()?;
  Ok(Add0 { a, x })
});

#[derive(Debug, Clone)]
pub struct Add0 {
  a: Numeric,
  x: ExprInst,
}
atomic_redirect!(Add0, x);
atomic_impl!(Add0, |Self { a, x }: &Self, _| {
  let b: Numeric = x.clone().try_into()?;
  Ok((*a + b).into())
});
