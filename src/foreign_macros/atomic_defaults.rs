#[allow(unused)] // for the doc comments
use crate::foreign::Atomic;

/// A macro that generates the straightforward, syntactically invariant part of implementing
/// [Atomic]. Implemented fns are [Atomic::as_any], [Atomic::definitely_eq] and [Atomic::hash].
/// 
/// It depends on [Eq] and [Hash]
#[macro_export]
macro_rules! atomic_defaults {
  () => {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn definitely_eq(&self, _other: &dyn std::any::Any) -> bool {
      _other.downcast_ref().map(|o| self == o).unwrap_or(false)
    }
    fn hash(&self, mut hasher: &mut dyn std::hash::Hasher) {
      <Self as std::hash::Hash>::hash(self, &mut hasher)
    }
  };
}