// PULL LOGISTICS BOUNDARY
//
// Specifying exactly what this module should be doing was an unexpectedly
// hard challenge. It is intended to encapsulate all pull logistics, but
// this definition is apparently prone to scope creep.
//
// Load files, preparse them to obtain a list of imports, follow these.
// Preparsing also returns the module tree and list of exported synbols
// for free, which is needed later so the output of preparsing is also
// attached to the module output.
//
// The module checks for IO errors, syntax errors, malformed imports and
// imports from missing files. All other errors must be checked later.
//
// Injection strategy:
// see whether names are valid in the injected tree for is_injected

mod load_source;
mod loaded_source;
mod preparse;
mod types;

pub use load_source::load_source;
pub use loaded_source::{LoadedSource, LoadedSourceTable};
pub use types::{PreExtra, PreFileExt, PreItem, PreMod, PreSubExt, Preparsed};
