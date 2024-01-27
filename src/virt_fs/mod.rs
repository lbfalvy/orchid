//! Abstractions and primitives to help define the namespace tree used by
//! Orchid.
//!
//! Although this may make it seem like the namespace tree is very flexible,
//! libraries are generally permitted and expected to hardcode their own
//! location in the tree, so it's up to the embedder to ensure that the flexible
//! structure retains the assumed location of all code.
mod common;
mod decl;
mod dir;
mod embed;
mod prefix;

pub use common::{CodeNotFound, FSResult, Loaded, VirtFS};
pub use decl::DeclTree;
pub use dir::DirNode;
pub use embed::EmbeddedFS;
pub use prefix::PrefixFS;
