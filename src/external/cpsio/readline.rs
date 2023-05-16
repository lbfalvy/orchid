use std::fmt::Debug;
use std::io::stdin;

use crate::external::runtime_error::RuntimeError;
use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::{Primitive, Literal};
use crate::representations::interpreted::{Clause, ExprInst};

/// Readln function
/// 
/// Next state: [Readln1]

#[derive(Clone)]
pub struct Readln2;
externfn_impl!(Readln2, |_: &Self, x: ExprInst| Ok(Readln1{x}));

/// Partially applied Readln function
/// 
/// Prev state: [Readln2]; Next state: [Readln0]

#[derive(Debug, Clone)]
pub struct Readln1{ x: ExprInst }
atomic_redirect!(Readln1, x);
atomic_impl!(Readln1, |Self{ x }: &Self, _| {
  let mut buf = String::new();
  stdin().read_line(&mut buf)
    .map_err(|e| RuntimeError::ext(e.to_string(), "reading from stdin"))?;
  buf.pop();
  Ok(Clause::Apply {
    f: x.clone(),
    x: Clause::P(Primitive::Literal(Literal::Str(buf))).wrap()
  })
});
