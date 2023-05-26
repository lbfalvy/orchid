#[allow(unused)] // for the doc comments
use crate::foreign::Atomic;

/// A macro that generates the straightforward, syntactically invariant part of
/// implementing [Atomic].
///
/// Currently implements
/// - [Atomic::as_any]
#[macro_export]
macro_rules! atomic_defaults {
  () => {
    fn as_any(&self) -> &dyn std::any::Any {
      self
    }
  };
}
