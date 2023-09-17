//! A type-agnostic interner
//!
//! Can be used to deduplicate various structures for fast equality comparisons.
//! The parser uses it to intern strings.
mod monotype;
mod multitype;
mod token;

pub use monotype::TypedInterner;
pub use multitype::Interner;
pub use token::Tok;
