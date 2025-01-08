//! The concept of a fallible merger

use never::Never;

/// Fallible, type-preserving variant of [std::ops::Add] implemented by a
/// variety of types for different purposes. Very broadly, if the operation
/// succeeds, the result should represent _both_ inputs.
pub trait Combine: Sized {
	/// Information about the failure
	type Error;

	/// Merge two values into a value that represents both, if this is possible.
	fn combine(self, other: Self) -> Result<Self, Self::Error>;
}

impl Combine for Never {
	type Error = Never;
	fn combine(self, _: Self) -> Result<Self, Self::Error> { match self {} }
}

impl Combine for () {
	type Error = Never;
	fn combine(self, (): Self) -> Result<Self, Self::Error> { Ok(()) }
}
