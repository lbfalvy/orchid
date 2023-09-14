//! An event queue other systems can use to trigger events on the main
//! interpreter thread. These events are handled when the Orchid code returns
//! `system::async::yield`, and may cause additional Orchid code to be executed
//! beyond being general Rust functions.
//! It also exposes timers.

mod system;

pub use system::{AsynchSystem, InfiniteBlock, MessagePort};
