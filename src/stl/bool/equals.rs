use std::fmt::Debug;

use super::super::assertion_error::AssertionError;
use super::super::litconv::with_lit;
use super::boolean::Boolean;
use crate::representations::interpreted::ExprInst;
use crate::representations::Literal;
use crate::{atomic_impl, atomic_redirect, externfn_impl};

/// Compares the inner values if
///
/// - both values are char,
/// - both are string,
/// - both are either uint or num
///
/// Next state: [Equals1]

#[derive(Clone)]
pub struct Equals2;
externfn_impl!(Equals2, |_: &Self, x: ExprInst| Ok(Equals1 { x }));

/// Prev state: [Equals2]; Next state: [Equals0]

#[derive(Debug, Clone)]
pub struct Equals1 {
  x: ExprInst,
}
atomic_redirect!(Equals1, x);
atomic_impl!(Equals1);
externfn_impl!(Equals1, |this: &Self, x: ExprInst| {
  with_lit(&this.x, |l| Ok(Equals0 { a: l.clone(), x }))
});

/// Prev state: [Equals1]
#[derive(Debug, Clone)]
pub struct Equals0 {
  a: Literal,
  x: ExprInst,
}
atomic_redirect!(Equals0, x);
atomic_impl!(Equals0, |Self { a, x }: &Self, _| {
  let eqls = with_lit(x, |l| {
    Ok(match (a, l) {
      (Literal::Char(c1), Literal::Char(c2)) => c1 == c2,
      (Literal::Num(n1), Literal::Num(n2)) => n1 == n2,
      (Literal::Str(s1), Literal::Str(s2)) => s1 == s2,
      (Literal::Uint(i1), Literal::Uint(i2)) => i1 == i2,
      (Literal::Num(n1), Literal::Uint(u1)) => *n1 == (*u1 as f64),
      (Literal::Uint(u1), Literal::Num(n1)) => *n1 == (*u1 as f64),
      (..) => AssertionError::fail(x.clone(), "the expected type")?,
    })
  })?;
  Ok(Boolean::from(eqls).to_atom_cls())
});
