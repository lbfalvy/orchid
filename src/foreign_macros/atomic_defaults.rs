#[allow(unused)] // for the doc comments
use crate::foreign::Atomic;

/// A macro that generates the straightforward, syntactically invariant part of
/// implementing [Atomic]. Implemented fns are [Atomic::as_any],
/// [Atomic::definitely_eq] and [Atomic::hash].
///
/// It depends on [Eq] and [Hash]
#[macro_export]
macro_rules! atomic_defaults {
  () => {
    fn as_any(&self) -> &dyn std::any::Any {
      self
    }
  };
}
