use std::fmt::Debug;
use std::io::{self, Write};
use std::rc::Rc;

use crate::external::litconv::with_str;
use crate::representations::PathSet;
use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::representations::interpreted::{Clause, ExprInst};

/// Print function
/// 
/// Next state: [Print1]

#[derive(Clone)]
pub struct Print2;
externfn_impl!(Print2, |_: &Self, x: ExprInst| Ok(Print1{x}));

/// Partially applied Print function
/// 
/// Prev state: [Print2]; Next state: [Print0]

#[derive(Debug, Clone)]
pub struct Print1{ x: ExprInst }
atomic_redirect!(Print1, x);
atomic_impl!(Print1, |Self{ x }: &Self, _| {
  with_str(x, |s| {
    print!("{}", s);
    io::stdout().flush().unwrap();
    Ok(Clause::Lambda {
      args: Some(PathSet{ steps: Rc::new(vec![]), next: None }),
      body: Clause::LambdaArg.wrap()
    })
  })
});
