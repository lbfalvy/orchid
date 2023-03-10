use std::fmt::Debug;
use std::hash::Hash;
use std::rc::Rc;

use crate::external::assertion_error::AssertionError;
use crate::representations::PathSet;
use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::interpreted::Clause;

use super::Boolean;

/// IfThenElse function
/// 
/// Next state: [IfThenElse0]

#[derive(Clone)]
pub struct IfThenElse1;
externfn_impl!(IfThenElse1, |_: &Self, c: Clause| {Ok(IfThenElse0{c})});

/// Partially applied IfThenElse function
/// 
/// Prev state: [IfThenElse1]; Next state: [IfThenElse0]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct IfThenElse0{ c: Clause }
atomic_redirect!(IfThenElse0, c);
atomic_impl!(IfThenElse0, |this: &Self| {
  let Boolean(b) = (&this.c).try_into()
    .map_err(|_| AssertionError::ext(this.c.clone(), "a boolean"))?;
  Ok(if b { Clause::Lambda {
    args: Some(PathSet { steps: Rc::new(vec![]), next: None }),
    body: Rc::new(Clause::Lambda {
      args: None,
      body: Rc::new(Clause::LambdaArg)
    })
  }} else { Clause::Lambda {
    args: None,
    body: Rc::new(Clause::Lambda {
      args: Some(PathSet { steps: Rc::new(vec![]), next: None }),
      body: Rc::new(Clause::LambdaArg)
    })
  }})
});