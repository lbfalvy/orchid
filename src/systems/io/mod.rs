//! System that allows Orchid to interact with trait objects of Rust's `Writer`
//! and with `BufReader`s of `Reader` trait objects

mod bindings;
// mod facade;
mod flow;
mod instances;
mod service;

// pub use facade::{io_system, IOStream, IOSystem};
pub use service::{Service, Stream, StreamTable};
