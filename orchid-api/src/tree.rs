use std::collections::HashMap;
use std::num::NonZeroU64;
use std::ops::Range;
use std::rc::Rc;

use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;

use crate::{ExprTicket, Expression, ExtHostReq, HostExtReq, OrcError, SysId, TStr, TStrv};

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
	/// A newly generated expression. The last place this is supposed to happen is
	/// in lexers, parsers and macros should have enumerable many outputs
	/// expressed as function calls.
	NewExpr(Expression),
	/// A pre-existing expression
	Handle(ExprTicket),
	/// ::
	NS(TStr, Box<TokenTree>),
	/// Line break.
	BR,
	/// ( Round parens ), [ Square brackets ] or { Curly braces }
	S(Paren, Vec<TokenTree>),
	/// A static compile-time error returned by failing lexers if
	/// the rest of the source is likely still meaningful. This is distinct from
	/// NewExpr(Bottom) because it fails in dead branches too.
	Bottom(Vec<OrcError>),
	/// A comment
	Comment(Rc<String>),
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
pub struct Member {
	pub name: TStr,
	pub exported: bool,
	pub kind: MemberKind,
	pub comments: Vec<TStr>,
}

#[derive(Clone, Debug, Coding)]
pub enum MemberKind {
	Const(Expression),
	Module(Module),
	Import(TStrv),
	Lazy(TreeId),
}

#[derive(Clone, Debug, Coding)]
pub struct Module {
	pub members: Vec<Member>,
}

/// Evaluate a lazy member. This call will only be issued to each system once.
#[derive(Clone, Copy, Debug, Coding, Hierarchy)]
#[extends(HostExtReq)]
pub struct GetMember(pub SysId, pub TreeId);
impl Request for GetMember {
	type Response = MemberKind;
}

/// This request can only be issued while the interpreter is running, so during
/// an atom call.
#[derive(Clone, Copy, Debug, Coding, Hierarchy)]
#[extends(ExtHostReq)]
pub struct LsModule(pub SysId, pub TStrv);
impl Request for LsModule {
	type Response = Result<ModuleInfo, LsModuleError>;
}

#[derive(Clone, Debug, Coding)]
pub enum LsModuleError {
	InvalidPath,
	IsConstant,
	TreeUnavailable,
}

#[derive(Clone, Debug, Coding)]
pub struct ModuleInfo {
	/// If the name isn't a canonical name, returns the true name.
	pub canonical: Option<TStrv>,
	/// List the names defined in this module
	pub members: HashMap<TStr, MemberInfo>,
}

#[derive(Clone, Copy, Debug, Coding)]
pub struct MemberInfo {
	/// true if the name is exported
	pub exported: bool,
	/// If it's imported, you can find the canonical name here
	pub canonical: Option<TStrv>,
	/// Whether the tree item is a constant value or a module
	pub kind: MemberInfoKind,
}

/// Indicates what kind of node a name refers to
#[derive(Clone, Copy, Debug, Coding)]
pub enum MemberInfoKind {
	/// has children obtained with [crate::LsModule]
	Module,
	/// has a value retrievable in [crate::ExpressionKind::Const]
	Constant,
}
