use std::collections::HashMap;

use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;
use ordered_float::NotNan;

use crate::atom::Atom;
use crate::expr::Expr;
use crate::intern::TStr;
use crate::location::SourceRange;
use crate::proto::HostExtReq;
use crate::system::SysId;

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding)]
pub struct TokenTree {
  token: Token,
  location: SourceRange,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding)]
pub enum Token {
  /// Lambda function. The number operates as an argument name
  Lambda(TStr, Vec<TokenTree>),
  Name(Vec<TStr>),
  S(Paren, Vec<TokenTree>),
  /// A placeholder in a macro. This variant is forbidden everywhere outside
  /// line parser output
  Ph(Placeholder),
  Atom(Atom),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding)]
pub struct Placeholder {
  name: TStr,
  kind: PlaceholderKind,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding)]
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

#[derive(Clone, Debug, Coding)]
pub enum Tree {
  Const(Expr),
  Mod(TreeModule),
  Rule(MacroRule),
}

#[derive(Clone, Debug, Coding)]
pub struct TreeModule {
  pub children: HashMap<String, Tree>,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(HostExtReq)]
pub struct GetConstTree(pub SysId);
impl Request for GetConstTree {
  type Response = TreeModule;
}
