use std::fmt::Display;

use super::super::litconv::with_str;
use crate::define_fn;
use crate::foreign::ExternError;

define_fn! {
  /// Takes a message, returns an [ExternError] unconditionally.
  pub Panic = |x| with_str(x, |s| Err(OrchidPanic(s.clone()).into_extern()))
}
/// An unrecoverable error in Orchid land. Because Orchid is lazy, this only
/// invalidates expressions that reference the one that generated it.
pub struct OrchidPanic(String);

impl Display for OrchidPanic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Orchid code panicked: {}", self.0)
  }
}

impl ExternError for OrchidPanic {}
