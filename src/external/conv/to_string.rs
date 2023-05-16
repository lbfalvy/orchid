
use std::fmt::Debug;

use crate::external::litconv::with_lit;
use crate::representations::{interpreted::ExprInst, Literal};
use crate::{atomic_impl, atomic_redirect, externfn_impl};

/// ToString a clause
/// 
/// Next state: [ToString0]

#[derive(Clone)]
pub struct ToString1;
externfn_impl!(ToString1, |_: &Self, x: ExprInst| Ok(ToString0{x}));

/// Applied ToString function
/// 
/// Prev state: [ToString1]

#[derive(Debug, Clone)]
pub struct ToString0{ x: ExprInst }
atomic_redirect!(ToString0, x);
atomic_impl!(ToString0, |Self{ x }: &Self, _| {
  let string = with_lit(x, |l| Ok(match l {
    Literal::Char(c) => c.to_string(),
    Literal::Uint(i) => i.to_string(),
    Literal::Num(n) => n.to_string(),
    Literal::Str(s) => s.clone()
  }))?;
  Ok(string.into())
});
