use std::ops::Range;

use orchid_api_derive::Coding;

use crate::{TStr, TStrv};

#[derive(Clone, Debug, Coding)]
pub enum Location {
	/// Location inaccessible. Locations are always debugging aids and never
	/// mandatory.
	None,
	/// Associated with a slot when wrapped in an expression.
	SlotTarget,
	/// Used in functions to denote the generated code that carries on the
	/// location of the call.
	Inherit,
	Gen(CodeGenInfo),
	/// Range and file
	SourceRange(SourceRange),
	/// Range only, file implied. Most notably used by parsers
	Range(Range<u32>),
}

#[derive(Clone, Debug, Coding)]
pub struct SourceRange {
	pub path: TStrv,
	pub range: Range<u32>,
}

#[derive(Clone, Debug, Coding)]
pub struct CodeGenInfo {
	pub generator: TStrv,
	pub details: TStr,
}
