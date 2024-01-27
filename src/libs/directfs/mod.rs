//! A rudimentary system exposing methods for Orchid to interact with the file
//! system. All paths are strings.
//!
//! The system depends on [crate::libs::scheduler] for scheduling blocking I/O
//! on a separate thread.
mod commands;
mod osstring;

pub use commands::DirectFS;
