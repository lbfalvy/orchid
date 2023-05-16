use std::fmt::Display;

use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::external::litconv::with_str;
use crate::representations::interpreted::ExprInst;
use crate::foreign::ExternError;

#[derive(Clone)]
pub struct Panic1;
externfn_impl!(Panic1, |_: &Self, x: ExprInst| Ok(Panic0{ x }));

#[derive(Debug, Clone)]
pub struct Panic0{ x: ExprInst }
atomic_redirect!(Panic0, x);
atomic_impl!(Panic0, |Self{ x }: &Self, _| {
  with_str(x, |s| {
    Err(OrchidPanic(s.clone()).into_extern())
  })
});

pub struct OrchidPanic(String);

impl Display for OrchidPanic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Orchid code panicked: {}", self.0)
  }
}

impl ExternError for OrchidPanic {}