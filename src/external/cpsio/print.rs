use std::fmt::Debug;
use std::hash::Hash;
use std::rc::Rc;

use crate::external::str::cls2str;
use crate::representations::PathSet;
use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::interpreted::Clause;

/// Print function
/// 
/// Next state: [Print1]

#[derive(Clone)]
pub struct Print2;
externfn_impl!(Print2, |_: &Self, c: Clause| {Ok(Print1{c})});

/// Partially applied Print function
/// 
/// Prev state: [Print2]; Next state: [Print0]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Print1{ c: Clause }
atomic_redirect!(Print1, c);
atomic_impl!(Print1, |Self{ c }: &Self| {
  let message = cls2str(&c)?;
  print!("{}", message);
  Ok(Clause::Lambda {
    args: Some(PathSet{ steps: Rc::new(vec![]), next: None }),
    body: Rc::new(Clause::LambdaArg)
  })
});
