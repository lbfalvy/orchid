use super::cls2str;

use std::fmt::Debug;
use std::hash::Hash;

use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::{Primitive, Literal};
use crate::representations::interpreted::Clause;

/// Concatenate function
/// 
/// Next state: [Concatenate1]

#[derive(Clone)]
pub struct Concatenate2;
externfn_impl!(Concatenate2, |_: &Self, c: Clause| {Ok(Concatenate1{c})});

/// Partially applied Concatenate function
/// 
/// Prev state: [Concatenate2]; Next state: [Concatenate0]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Concatenate1{ c: Clause }
atomic_redirect!(Concatenate1, c);
atomic_impl!(Concatenate1);
externfn_impl!(Concatenate1, |this: &Self, c: Clause| {
  let a: String = cls2str(&this.c)?.clone();
  Ok(Concatenate0{ a, c })
});

/// Fully applied Concatenate function.
/// 
/// Prev state: [Concatenate1]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Concatenate0 { a: String, c: Clause }
atomic_redirect!(Concatenate0, c);
atomic_impl!(Concatenate0, |Self{ a, c }: &Self| {
  let b: &String = cls2str(c)?;
  Ok(Clause::P(Primitive::Literal(Literal::Str(a.to_owned() + b))))
});
