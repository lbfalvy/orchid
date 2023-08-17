//! A simplified set of commands each grouping a large subset of the operations
//! exposed by Orchid to make writing embeddings faster in the typical case.

mod environment;
mod pre_macro;
mod process;
mod system;

pub use environment::{CompiledEnv, Environment};
pub use pre_macro::{MacroTimeout, PreMacro};
pub use process::Process;
pub use system::{IntoSystem, MissingSystemCode, System};
