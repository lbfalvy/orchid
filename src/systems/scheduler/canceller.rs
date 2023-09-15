use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::foreign::InertAtomic;

/// A single-fire thread-safe boolean flag with relaxed ordering
#[derive(Debug, Clone)]
pub struct Canceller(Arc<AtomicBool>);
impl InertAtomic for Canceller {
  fn type_str() -> &'static str { "a canceller" }
}

impl Canceller {
  /// Create a new canceller
  pub fn new() -> Self { Canceller(Arc::new(AtomicBool::new(false))) }

  /// Check whether the operation has been cancelled
  pub fn is_cancelled(&self) -> bool { self.0.load(Ordering::Relaxed) }

  /// Cancel the operation
  pub fn cancel(&self) { self.0.store(true, Ordering::Relaxed) }
}

impl Default for Canceller {
  fn default() -> Self { Self::new() }
}
