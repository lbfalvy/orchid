//! Flag for cancelling scheduled operations

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A single-fire thread-safe boolean flag with relaxed ordering
#[derive(Debug, Clone)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
  /// Create a new canceller
  pub fn new() -> Self { CancelFlag(Arc::new(AtomicBool::new(false))) }

  /// Check whether the operation has been cancelled
  pub fn is_cancelled(&self) -> bool { self.0.load(Ordering::Relaxed) }

  /// Cancel the operation
  pub fn cancel(&self) { self.0.store(true, Ordering::Relaxed) }
}

impl Default for CancelFlag {
  fn default() -> Self { Self::new() }
}
