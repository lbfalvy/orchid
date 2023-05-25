use std::fmt::Display;

use crate::external::litconv::with_str;
use crate::foreign::ExternError;
use crate::representations::interpreted::ExprInst;
use crate::{atomic_impl, atomic_redirect, externfn_impl};

/// Takes a message, returns an [ExternError] unconditionally.
///
/// Next state: [Panic0]
#[derive(Clone)]
pub struct Panic1;
externfn_impl!(Panic1, |_: &Self, x: ExprInst| Ok(Panic0 { x }));

/// Prev state: [Panic1]
#[derive(Debug, Clone)]
pub struct Panic0 {
  x: ExprInst,
}
atomic_redirect!(Panic0, x);
atomic_impl!(Panic0, |Self { x }: &Self, _| {
  with_str(x, |s| Err(OrchidPanic(s.clone()).into_extern()))
});

/// An unrecoverable error in Orchid land. Of course, because Orchid is lazy, it
/// only applies to the expressions that use the one that generated it.
pub struct OrchidPanic(String);

impl Display for OrchidPanic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Orchid code panicked: {}", self.0)
  }
}

impl ExternError for OrchidPanic {}
