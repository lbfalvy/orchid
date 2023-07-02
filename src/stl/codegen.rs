//! Utilities for generating Orchid code in Rust

use std::rc::Rc;

use crate::interpreted::{Clause, ExprInst};
use crate::{PathSet, Side};

/// Convert a rust Option into an Orchid Option
pub fn orchid_opt(x: Option<ExprInst>) -> Clause {
  if let Some(x) = x { some(x) } else { none() }
}

/// Constructs an instance of the orchid value Some wrapping the given
/// [ExprInst]
fn some(x: ExprInst) -> Clause {
  Clause::Lambda {
    args: None,
    body: Clause::Lambda {
      args: Some(PathSet { steps: Rc::new(vec![Side::Left]), next: None }),
      body: Clause::Apply { f: Clause::LambdaArg.wrap(), x }.wrap(),
    }
    .wrap(),
  }
}

/// Constructs an instance of the orchid value None
fn none() -> Clause {
  Clause::Lambda {
    args: Some(PathSet { steps: Rc::new(vec![]), next: None }),
    body: Clause::Lambda { args: None, body: Clause::LambdaArg.wrap() }.wrap(),
  }
}

/// Define a clause that can be called with a callback and passes the provided
/// values to the callback in order.
pub fn tuple(data: Vec<ExprInst>) -> Clause {
  Clause::Lambda {
    args: Some(PathSet {
      next: None,
      steps: Rc::new(data.iter().map(|_| Side::Left).collect()),
    }),
    body: data
      .into_iter()
      .fold(Clause::LambdaArg.wrap(), |f, x| Clause::Apply { f, x }.wrap()),
  }
}
