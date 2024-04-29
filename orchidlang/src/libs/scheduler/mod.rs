//! A generic utility to sequence long blocking mutations that require a mutable
//! reference to a shared resource.

mod busy;
pub mod cancel_flag;
mod id_map;
pub mod system;
pub mod thread_pool;
