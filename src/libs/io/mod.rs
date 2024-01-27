//! System that allows Orchid to interact with trait objects of Rust's `Writer`
//! and with `BufReader`s of `Reader` trait objects.
//!
//! You can pass standard streams during initialization, the stllib expects
//! `stdin`, `stdout` and `stderr`. This system depends on
//! [crate::libs::scheduler] to run blocking I/O operations off-thread, which in
//! turn depends on [crate::libs::asynch] to process results on the main thread,
//! and [crate::libs::std] for `std::panic`.
//!
//! ```
//! use orchidlang::libs::asynch::system::AsynchSystem;
//! use orchidlang::libs::scheduler::system::SeqScheduler;
//! use orchidlang::libs::std::std_system::StdConfig;
//! use orchidlang::libs::io::{IOService, Stream};
//! use orchidlang::facade::loader::Loader;
//! use std::io::BufReader;
//! 
//! 
//! let mut asynch = AsynchSystem::new();
//! let scheduler = SeqScheduler::new(&mut asynch);
//! let std_streams = [
//!   ("stdin", Stream::Source(BufReader::new(Box::new(std::io::stdin())))),
//!   ("stdout", Stream::Sink(Box::new(std::io::stdout()))),
//!   ("stderr", Stream::Sink(Box::new(std::io::stderr()))),
//! ];
//! let env = Loader::new()
//!   .add_system(StdConfig { impure: false })
//!   .add_system(asynch)
//!   .add_system(scheduler.clone())
//!   .add_system(IOService::new(scheduler.clone(), std_streams));
//! ```

mod bindings;
mod flow;
pub(super) mod instances;
mod service;

pub use service::{IOService, Sink, Source, Stream};
