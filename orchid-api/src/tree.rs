use std::num::NonZeroU64;
use std::ops::Range;
use std::sync::Arc;

use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;
use ordered_float::NotNan;

use crate::{
	Atom, Expression, HostExtReq, Location, MacroBlock, OrcError, Placeholder, SysId, TStr, TStrv,
};

/// A token tree from a lexer recursion request. Its lifetime is the lex call,
/// the lexer can include it in its output or discard it by implication.
///
/// Similar to [crate::expr::ExprTicket] in that it represents a token tree the
/// lifetime of which is managed by the interpreter, and as such should probably
/// not be exposed to libraries directly but rather wrapped in a
/// lifetime-controlled abstraction.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Coding)]
pub struct TreeTicket(pub NonZeroU64);

#[derive(Clone, Debug, Coding)]
pub struct TokenTree {
	pub token: Token,
	pub range: Range<u32>,
}

#[derive(Clone, Debug, Coding)]
pub enum Token {
	/// Lambda function head, from the opening \ until the beginning of the body.
	LambdaHead(Vec<TokenTree>),
	/// A name segment or an operator.
	Name(TStr),
	/// ::
	NS,
	/// Line break.
	BR,
	/// ( Round parens ), [ Square brackets ] or { Curly braces }
	S(Paren, Vec<TokenTree>),
	/// A new atom
	Atom(Atom),
	/// Anchor to insert a subtree
	Slot(TreeTicket),
	/// A static compile-time error returned by failing lexers if
	/// the rest of the source is likely still meaningful
	Bottom(Vec<OrcError>),
	/// A comment
	Comment(Arc<String>),
	/// Placeholder
	Ph(Placeholder),
	/// Macro block head
	Macro(Option<NotNan<f64>>),
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Coding)]
pub enum Paren {
	Round,
	Square,
	Curly,
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Coding)]
pub struct TreeId(pub NonZeroU64);

#[derive(Clone, Debug, Coding)]
pub struct Item {
	pub location: Location,
	pub comments: Vec<Comment>,
	pub kind: ItemKind,
}

#[derive(Clone, Debug, Coding)]
pub enum ItemKind {
	Member(Member),
	Macro(MacroBlock),
	Export(TStr),
	Import(TStrv),
}

#[derive(Clone, Debug, Coding)]
pub struct Comment {
	pub text: TStr,
	pub location: Location,
}

#[derive(Clone, Debug, Coding)]
pub struct Member {
	pub name: TStr,
	pub kind: MemberKind,
}

#[derive(Clone, Debug, Coding)]
pub enum MemberKind {
	Const(Expression),
	Module(Module),
	Lazy(TreeId),
}

#[derive(Clone, Debug, Coding)]
pub struct Module {
	pub items: Vec<Item>,
}

#[derive(Clone, Copy, Debug, Coding, Hierarchy)]
#[extends(HostExtReq)]
pub struct GetMember(pub SysId, pub TreeId);
impl Request for GetMember {
	type Response = MemberKind;
}
