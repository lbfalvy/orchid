use std::fmt::Debug;
use std::rc::Rc;

use super::Boolean;
use crate::external::assertion_error::AssertionError;
use crate::representations::interpreted::{Clause, ExprInst};
use crate::representations::PathSet;
use crate::{atomic_impl, atomic_redirect, externfn_impl};

/// Takes a boolean and two branches, runs the first if the bool is true, the
/// second if it's false.
///
/// Next state: [IfThenElse0]
#[derive(Clone)]
pub struct IfThenElse1;
externfn_impl!(IfThenElse1, |_: &Self, x: ExprInst| Ok(IfThenElse0 { x }));

/// Prev state: [IfThenElse1]
#[derive(Debug, Clone)]
pub struct IfThenElse0 {
  x: ExprInst,
}
atomic_redirect!(IfThenElse0, x);
atomic_impl!(IfThenElse0, |this: &Self, _| {
  let Boolean(b) = this
    .x
    .clone()
    .try_into()
    .map_err(|_| AssertionError::ext(this.x.clone(), "a boolean"))?;
  Ok(if b {
    Clause::Lambda {
      args: Some(PathSet { steps: Rc::new(vec![]), next: None }),
      body: Clause::Lambda { args: None, body: Clause::LambdaArg.wrap() }
        .wrap(),
    }
  } else {
    Clause::Lambda {
      args: None,
      body: Clause::Lambda {
        args: Some(PathSet { steps: Rc::new(vec![]), next: None }),
        body: Clause::LambdaArg.wrap(),
      }
      .wrap(),
    }
  })
});
