use std::fmt::Debug;

use crate::external::litconv::with_lit;
use crate::representations::{interpreted::ExprInst, Literal};
use crate::{atomic_impl, atomic_redirect, externfn_impl};

use super::super::assertion_error::AssertionError;
use super::boolean::Boolean;

/// Equals function
/// 
/// Next state: [Equals1]

#[derive(Clone)]
pub struct Equals2;
externfn_impl!(Equals2, |_: &Self, x: ExprInst| Ok(Equals1{x}));

/// Partially applied Equals function
/// 
/// Prev state: [Equals2]; Next state: [Equals0]

#[derive(Debug, Clone)]
pub struct Equals1{ x: ExprInst }
atomic_redirect!(Equals1, x);
atomic_impl!(Equals1);
externfn_impl!(Equals1, |this: &Self, x: ExprInst| {
  with_lit(&this.x, |l| Ok(Equals0{ a: l.clone(), x }))
});

/// Fully applied Equals function.
/// 
/// Prev state: [Equals1]

#[derive(Debug, Clone)]
pub struct Equals0 { a: Literal, x: ExprInst }
atomic_redirect!(Equals0, x);
atomic_impl!(Equals0, |Self{ a, x }: &Self| {
  let eqls = with_lit(x, |l| Ok(match (a, l) {
    (Literal::Char(c1), Literal::Char(c2)) => c1 == c2,
    (Literal::Num(n1), Literal::Num(n2)) => n1 == n2,
    (Literal::Str(s1), Literal::Str(s2)) => s1 == s2,
    (Literal::Uint(i1), Literal::Uint(i2)) => i1 == i2,
    (_, _) => AssertionError::fail(x.clone(), "the expected type")?,
  }))?;
  Ok(Boolean::from(eqls).to_atom_cls())
});
