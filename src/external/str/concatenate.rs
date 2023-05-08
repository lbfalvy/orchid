use std::fmt::Debug;

use crate::external::litconv::with_str;
use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::{Primitive, Literal};
use crate::representations::interpreted::{Clause, ExprInst};

/// Concatenate function
/// 
/// Next state: [Concatenate1]

#[derive(Clone)]
pub struct Concatenate2;
externfn_impl!(Concatenate2, |_: &Self, c: ExprInst| Ok(Concatenate1{c}));

/// Partially applied Concatenate function
/// 
/// Prev state: [Concatenate2]; Next state: [Concatenate0]

#[derive(Debug, Clone)]
pub struct Concatenate1{ c: ExprInst }
atomic_redirect!(Concatenate1, c);
atomic_impl!(Concatenate1);
externfn_impl!(Concatenate1, |this: &Self, c: ExprInst| {
  with_str(&this.c, |a| Ok(Concatenate0{ a: a.clone(), c }))
});

/// Fully applied Concatenate function.
/// 
/// Prev state: [Concatenate1]

#[derive(Debug, Clone)]
pub struct Concatenate0 { a: String, c: ExprInst }
atomic_redirect!(Concatenate0, c);
atomic_impl!(Concatenate0, |Self{ a, c }: &Self| {
  with_str(c, |b| Ok(Clause::P(Primitive::Literal(
    Literal::Str(a.to_owned() + b)
  ))))
});
