use std::fmt::Debug;
use std::hash::Hash;

use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::foreign::Atom;
use crate::representations::{Primitive, Literal};
use crate::representations::interpreted::Clause;

use super::super::assertion_error::AssertionError;
use super::boolean::Boolean;

/// Equals function
/// 
/// Next state: [Equals1]

#[derive(Clone)]
pub struct Equals2;
externfn_impl!(Equals2, |_: &Self, c: Clause| {Ok(Equals1{c})});

/// Partially applied Equals function
/// 
/// Prev state: [Equals2]; Next state: [Equals0]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Equals1{ c: Clause }
atomic_redirect!(Equals1, c);
atomic_impl!(Equals1);
externfn_impl!(Equals1, |this: &Self, c: Clause| {
  let a: Literal = this.c.clone().try_into()
    .map_err(|_| AssertionError::ext(this.c.clone(), "a primitive"))?;
  Ok(Equals0{ a, c })
});

/// Fully applied Equals function.
/// 
/// Prev state: [Equals1]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Equals0 { a: Literal, c: Clause }
atomic_redirect!(Equals0, c);
atomic_impl!(Equals0, |Self{ a, c }: &Self| {
  let b: Literal = c.clone().try_into()
    .map_err(|_| AssertionError::ext(c.clone(), "a literal value"))?;
  let eqls = match (a, b) {
    (Literal::Char(c1), Literal::Char(c2)) => *c1 == c2,
    (Literal::Num(n1), Literal::Num(n2)) => *n1 == n2,
    (Literal::Str(s1), Literal::Str(s2)) => *s1 == s2,
    (Literal::Uint(i1), Literal::Uint(i2)) => *i1 == i2,
    (_, _) => AssertionError::fail(c.clone(), "the expected type")?,
  };
  Ok(Clause::P(Primitive::Atom(Atom::new(Boolean::from(eqls)))))
});
