use std::collections::HashMap;
use std::num::NonZeroU64;
use std::ops::Range;

use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;
use ordered_float::NotNan;

use crate::atom::LocalAtom;
use crate::error::ProjErrOrRef;
use crate::expr::Expr;
use crate::interner::TStr;
use crate::proto::HostExtReq;
use crate::system::SysId;

/// A token tree from a lexer recursion request. Its lifetime is the lex call,
/// the lexer can include it in its output or discard it by implication.
///
/// Similar to [crate::expr::ExprTicket] in that it represents a token tree the
/// lifetime of which is managed by the interpreter.
pub type TreeTicket = NonZeroU64;

#[derive(Clone, Debug, Coding)]
pub struct TokenTree {
  pub token: Token,
  pub range: Range<u32>,
}

#[derive(Clone, Debug, Coding)]
pub enum Token {
  /// Lambda function. The number operates as an argument name
  Lambda(Vec<TokenTree>, Vec<TokenTree>),
  Name(Vec<TStr>),
  S(Paren, Vec<TokenTree>),
  /// A placeholder in a macro. This variant is forbidden everywhere outside
  /// line parser output
  Ph(Placeholder),
  Atom(LocalAtom),
  Slot(TreeTicket),
  /// A static compile-time error returned by erroring lexers if
  /// the rest of the source is likely still meaningful
  Bottom(ProjErrOrRef),
}

#[derive(Clone, Debug, Coding)]
pub struct Placeholder {
  pub name: TStr,
  pub kind: PlaceholderKind,
}

#[derive(Clone, Debug, Coding)]
pub enum PlaceholderKind {
  Scalar,
  Name,
  Vector { nonzero: bool, priority: u8 },
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding)]
pub enum Paren {
  Round,
  Square,
  Curly,
}

#[derive(Clone, Debug, Coding)]
pub struct MacroRule {
  pub pattern: Vec<TokenTree>,
  pub priority: NotNan<f64>,
  pub template: Vec<TokenTree>,
}

pub type TreeId = NonZeroU64;

#[derive(Clone, Debug, Coding)]
pub enum Tree {
  Const(Expr),
  Mod(TreeModule),
  Rule(MacroRule),
  Lazy(TreeId),
}

#[derive(Clone, Debug, Coding)]
pub struct TreeModule {
  pub children: HashMap<String, Tree>,
}

#[derive(Clone, Copy, Debug, Coding, Hierarchy)]
#[extends(HostExtReq)]
pub struct GetConstTree(pub SysId, pub TreeId);
impl Request for GetConstTree {
  type Response = Tree;
}
