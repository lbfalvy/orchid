use std::fmt::Debug;
use std::io::stdin;
use std::rc::Rc;
use std::hash::Hash;

use crate::external::runtime_error::RuntimeError;
use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::{Primitive, Literal};
use crate::representations::interpreted::Clause;

/// Readln function
/// 
/// Next state: [Readln1]

#[derive(Clone)]
pub struct Readln2;
externfn_impl!(Readln2, |_: &Self, c: Clause| {Ok(Readln1{c})});

/// Partially applied Readln function
/// 
/// Prev state: [Readln2]; Next state: [Readln0]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Readln1{ c: Clause }
atomic_redirect!(Readln1, c);
atomic_impl!(Readln1, |Self{ c }: &Self| {
  let mut buf = String::new();
  stdin().read_line(&mut buf).map_err(|e| RuntimeError::ext(e.to_string(), "reading from stdin"))?;
  buf.pop();
  Ok(Clause::Apply {
    f: Rc::new(c.clone()),
    x: Rc::new(Clause::P(Primitive::Literal(Literal::Str(buf)))),
    id: 0
  })
});
