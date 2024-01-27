//! An event queue other systems can use to trigger events on the main
//! interpreter thread. These events are handled when the Orchid code returns
//! `system::async::yield`, and may cause additional Orchid code to be executed
//! beyond being general Rust functions.
//! It also exposes timers.

pub mod poller;
pub mod system;
mod delete_cell;
