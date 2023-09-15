//! A generic utility to sequence long blocking mutations that require a mutable
//! reference to a shared resource.

mod busy;
mod canceller;
mod system;

pub use canceller::Canceller;
pub use system::{SealedOrTaken, SeqScheduler, SharedHandle, SharedState};
