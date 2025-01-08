use std::collections::HashMap;
use std::num::NonZeroU64;

use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;
use ordered_float::NotNan;

use crate::{
	Atom, Comment, ExtHostReq, HostExtReq, Location, OrcResult, Paren, ParsId, SysId, TStr, TStrv,
};

#[derive(Clone, Copy, Debug, Coding, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MacroTreeId(pub NonZeroU64);

#[derive(Clone, Debug, Coding)]
pub struct MacroTree {
	pub location: Location,
	pub token: MacroToken,
}

#[derive(Clone, Debug, Coding)]
pub enum MacroToken {
	S(Paren, Vec<MacroTree>),
	Name(TStrv),
	Slot(MacroTreeId),
	Lambda(Vec<MacroTree>, Vec<MacroTree>),
	Ph(Placeholder),
	Atom(Atom),
}

#[derive(Clone, Debug, Coding)]
pub struct MacroBlock {
	pub priority: Option<NotNan<f64>>,
	pub rules: Vec<MacroRule>,
}

#[derive(Clone, Debug, Coding)]
pub struct MacroRule {
	pub location: Location,
	pub comments: Vec<Comment>,
	pub pattern: Vec<MacroTree>,
	pub id: MacroId,
}

/// A specific macro rule with a specific pattern across invocations
#[derive(Clone, Copy, Debug, Coding, PartialEq, Eq, Hash)]
pub struct MacroId(pub NonZeroU64);

/// After a pattern matches, this call executes the body of the macro. This
/// request returns None if an inner nested request raised an exception
#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(HostExtReq)]
pub struct ApplyMacro {
	pub sys: SysId,
	pub id: MacroId,
	/// Recursion token
	pub run_id: ParsId,
	/// Must contain exactly the keys that were specified as placeholders in the
	/// pattern
	pub params: HashMap<TStr, Vec<MacroTree>>,
}
impl Request for ApplyMacro {
	type Response = Option<OrcResult<Vec<MacroTree>>>;
}

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(ExtHostReq)]
pub struct RunMacros {
	pub run_id: ParsId,
	pub query: Vec<MacroTree>,
}
impl Request for RunMacros {
	type Response = Option<Vec<MacroTree>>;
}

#[derive(Clone, Debug, Coding)]
pub struct Placeholder {
	pub name: TStr,
	pub kind: PhKind,
}

#[derive(Clone, Copy, Debug, Coding)]
pub enum PhKind {
	Scalar,
	Vector { priority: u8, at_least_one: bool },
}
