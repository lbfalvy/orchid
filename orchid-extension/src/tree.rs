use std::ops::Range;

use ahash::HashMap;
use dyn_clone::{clone_box, DynClone};
use itertools::Itertools;
use orchid_api::tree::{
  MacroRule, Paren, PlaceholderKind, Token, TokenTree, Tree, TreeId, TreeModule,
  TreeTicket,
};
use orchid_base::interner::intern;
use orchid_base::location::Pos;
use orchid_base::name::VName;
use orchid_base::tokens::OwnedPh;
use ordered_float::NotNan;
use trait_set::trait_set;

use crate::atom::AtomFactory;
use crate::conv::ToExpr;
use crate::error::{err_or_ref_to_api, ProjectErrorObj};
use crate::expr::GenExpr;
use crate::system::DynSystem;

#[derive(Clone)]
pub struct OwnedTokTree {
  pub tok: OwnedTok,
  pub range: Range<u32>,
}
impl OwnedTokTree {
  pub fn into_api(self, sys: &dyn DynSystem) -> TokenTree {
    TokenTree { token: self.tok.into_api(sys), range: self.range }
  }
}

pub fn ph(s: &str) -> OwnedPh {
  match s.strip_prefix("..") {
    Some(v_tail) => {
      let (mid, priority) = match v_tail.split_once(':') {
        Some((h, t)) => (h, t.parse().expect("priority not an u8")),
        None => (v_tail, 0),
      };
      let (name, nonzero) = match mid.strip_prefix(".$") {
        Some(name) => (name, true),
        None => (mid.strip_prefix('$').expect("Invalid placeholder"), false),
      };
      if konst::string::starts_with(name, "_") {
        panic!("Names starting with an underscore indicate a single-name scalar placeholder")
      }
      OwnedPh { name: intern(name), kind: PlaceholderKind::Vector { nonzero, priority } }
    },
    None => match konst::string::strip_prefix(s, "$_") {
      Some(name) => OwnedPh { name: intern(name), kind: PlaceholderKind::Name },
      None => match konst::string::strip_prefix(s, "$") {
        None => panic!("Invalid placeholder"),
        Some(name) => OwnedPh { name: intern(name), kind: PlaceholderKind::Scalar },
      },
    },
  }
}

#[derive(Clone)]
pub enum OwnedTok {
  Lambda(Vec<OwnedTokTree>, Vec<OwnedTokTree>),
  Name(VName),
  S(Paren, Vec<OwnedTokTree>),
  Atom(AtomFactory),
  Slot(TreeTicket),
  Ph(OwnedPh),
  Bottom(ProjectErrorObj),
}
impl OwnedTok {
  pub fn at(self, range: Range<u32>) -> OwnedTokTree { OwnedTokTree { tok: self, range } }
  pub fn into_api(self, sys: &dyn DynSystem) -> Token {
    match self {
      Self::Lambda(x, body) => Token::Lambda(
        x.into_iter().map(|tt| tt.into_api(sys)).collect_vec(),
        body.into_iter().map(|tt| tt.into_api(sys)).collect_vec(),
      ),
      Self::Name(n) => Token::Name(n.into_iter().map(|t| t.marker()).collect_vec()),
      Self::Ph(ph) => Token::Ph(ph.to_api()),
      Self::S(p, body) => Token::S(p, body.into_iter().map(|tt| tt.into_api(sys)).collect_vec()),
      Self::Slot(tk) => Token::Slot(tk),
      Self::Atom(at) => Token::Atom(at.build(sys)),
      Self::Bottom(err) => Token::Bottom(err_or_ref_to_api(err)),
    }
  }
}

#[derive(Clone)]
pub struct GenMacro {
  pub pattern: Vec<OwnedTokTree>,
  pub priority: NotNan<f64>,
  pub template: Vec<OwnedTokTree>,
}

pub fn tokv_into_api(
  tokv: impl IntoIterator<Item = OwnedTokTree>,
  sys: &dyn DynSystem,
) -> Vec<TokenTree> {
  tokv.into_iter().map(|tok| tok.into_api(sys)).collect_vec()
}

pub fn wrap_tokv(items: Vec<OwnedTokTree>, range: Range<u32>) -> OwnedTokTree {
  match items.len() {
    1 => items.into_iter().next().unwrap(),
    _ => OwnedTok::S(Paren::Round, items).at(range),
  }
}

#[derive(Clone)]
pub struct GenTree {
  pub item: GenItem,
  pub location: Pos,
}
impl GenTree {
  pub fn cnst(gc: impl ToExpr) -> Self { GenItem::Const(gc.to_expr()).at(Pos::Inherit) }
  pub fn module<'a>(entries: impl IntoIterator<Item = (&'a str, GenTree)>) -> Self {
    GenItem::Mod(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect()).at(Pos::Inherit)
  }
  pub fn rule(
    prio: f64,
    pat: impl IntoIterator<Item = OwnedTokTree>,
    tpl: impl IntoIterator<Item = OwnedTokTree>,
  ) -> Self {
    GenItem::Rule(GenMacro {
      pattern: pat.into_iter().collect(),
      priority: NotNan::new(prio).expect("expected to be static"),
      template: tpl.into_iter().collect(),
    })
    .at(Pos::Inherit)
  }
  pub fn into_api(
    self,
    sys: &dyn DynSystem,
    with_lazy: &mut impl FnMut(LazyTreeFactory) -> TreeId,
  ) -> Tree {
    match self.item {
      GenItem::Const(gc) => Tree::Const(gc.into_api(sys)),
      GenItem::Rule(GenMacro { pattern, priority, template }) => Tree::Rule(MacroRule {
        pattern: tokv_into_api(pattern, sys),
        priority,
        template: tokv_into_api(template, sys),
      }),
      GenItem::Mod(entv) => Tree::Mod(TreeModule {
        children: entv
          .into_iter()
          .map(|(name, tree)| (name.to_string(), tree.into_api(sys, with_lazy)))
          .collect(),
      }),
      GenItem::Lazy(cb) => Tree::Lazy(with_lazy(cb)),
    }
  }
}

trait_set! {
  trait LazyTreeCallback = FnMut() -> GenTree + Send + Sync + DynClone
}
pub struct LazyTreeFactory(Box<dyn LazyTreeCallback>);
impl LazyTreeFactory {
  pub fn new(cb: impl FnMut() -> GenTree + Send + Sync + Clone + 'static) -> Self {
    Self(Box::new(cb))
  }
  pub fn build(&mut self) -> GenTree { (self.0)() }
}
impl Clone for LazyTreeFactory {
  fn clone(&self) -> Self { Self(clone_box(&*self.0)) }
}

#[derive(Clone)]
pub enum GenItem {
  Const(GenExpr),
  Mod(HashMap<String, GenTree>),
  Rule(GenMacro),
  Lazy(LazyTreeFactory),
}
impl GenItem {
  pub fn at(self, position: Pos) -> GenTree { GenTree { item: self, location: position } }
}
