//! A type-agnostic interner
//!
//! Can be used to deduplicate various structures for fast equality comparisons.
//! The parser uses it to intern strings.
mod display;
mod monotype;
mod multitype;
mod token;

pub use display::{DisplayBundle, InternedDisplay};
pub use monotype::TypedInterner;
pub use multitype::Interner;
pub use token::Tok;

/// A symbol, nsname, nname or namespaced name is a sequence of namespaces
/// and an identifier. The [Vec] can never be empty.
///
/// Throughout different stages of processing, these names can be
///
/// - local names to be prefixed with the current module
/// - imported names starting with a segment
///   - ending a single import or
///   - defined in one of the glob imported modules
/// - absolute names
pub type Sym = Tok<Vec<Tok<String>>>;
