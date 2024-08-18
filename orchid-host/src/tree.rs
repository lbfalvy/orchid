use std::sync::{Mutex, OnceLock};

use itertools::Itertools;
use never::Never;
use orchid_base::error::OrcRes;
use orchid_base::interner::{deintern, Tok};
use orchid_base::location::Pos;
use orchid_base::name::Sym;
use orchid_base::parse::{Comment, CompName};
use orchid_base::tree::{ttv_from_api, TokTree, Token};
use ordered_float::NotNan;

use crate::api;
use crate::expr::RtExpr;
use crate::extension::{AtomHand, System};

pub type ParsTokTree = TokTree<'static, AtomHand, Never>;
pub type ParsTok = Token<'static, AtomHand, Never>;

#[derive(Debug)]
pub struct Item {
  pub pos: Pos,
  pub comments: Vec<Comment>,
  pub kind: ItemKind,
}

#[derive(Debug)]
pub enum ItemKind {
  Raw(Vec<ParsTokTree>),
  Member(Member),
  Export(Tok<String>),
  Rule(Macro),
  Import(CompName),
}

impl Item {
  pub fn from_api(tree: api::Item, sys: &System) -> Self {
    let kind = match tree.kind {
      api::ItemKind::Raw(tokv) => ItemKind::Raw(ttv_from_api(tokv, &mut ())),
      api::ItemKind::Member(m) => ItemKind::Member(Member::from_api(m, sys)),
      api::ItemKind::Rule(r) => ItemKind::Rule(Macro::from_api(r)),
      api::ItemKind::Import(i) => ItemKind::Import(CompName::from_api(i)),
      api::ItemKind::Export(e) => ItemKind::Export(deintern(e)),
    };
    let comments = tree
      .comments
      .into_iter()
      .map(|(text, l)| Comment { text, pos: Pos::from_api(&l) })
      .collect_vec();
    Self { pos: Pos::from_api(&tree.location), comments, kind }
  }
}

#[derive(Debug)]
pub struct Member {
  pub exported: bool,
  pub name: Tok<String>,
  pub kind: OnceLock<MemberKind>,
  pub lazy: Mutex<Option<LazyMemberHandle>>,
}
impl Member {
  pub fn from_api(api::Member { exported: public, name, kind }: api::Member, sys: &System) -> Self {
    let (kind, lazy) = match kind {
      api::MemberKind::Const(c) =>
        (OnceLock::from(MemberKind::PreCnst(RtExpr::from_api(c, sys))), None),
      api::MemberKind::Module(m) =>
        (OnceLock::from(MemberKind::Mod(Module::from_api(m, sys))), None),
      api::MemberKind::Lazy(id) => (OnceLock::new(), Some(LazyMemberHandle(id, sys.clone()))),
    };
    Member { exported: public, name: deintern(name), kind, lazy: Mutex::new(lazy) }
  }
  pub fn new(public: bool, name: Tok<String>, kind: MemberKind) -> Self {
    Member { exported: public, name, kind: OnceLock::from(kind), lazy: Mutex::default() }
  }
}

#[derive(Debug)]
pub enum MemberKind {
  Const(Vec<ParsTokTree>),
  PreCnst(RtExpr),
  Mod(Module),
}

#[derive(Debug)]
pub struct Module {
  pub imports: Vec<Sym>,
  pub items: Vec<Item>,
}
impl Module {
  pub fn from_api(m: api::Module, sys: &System) -> Self {
    Self {
      imports: m.imports.into_iter().map(|m| Sym::from_tok(deintern(m)).unwrap()).collect_vec(),
      items: m.items.into_iter().map(|i| Item::from_api(i, sys)).collect_vec(),
    }
  }
}

#[derive(Debug)]
pub struct Macro {
  pub priority: NotNan<f64>,
  pub pattern: Vec<ParsTokTree>,
  pub template: Vec<ParsTokTree>,
}
impl Macro {
  pub fn from_api(m: api::Macro) -> Self {
    Self {
      priority: m.priority,
      pattern: ttv_from_api(m.pattern, &mut ()),
      template: ttv_from_api(m.template, &mut ()),
    }
  }
}

#[derive(Debug)]
pub struct LazyMemberHandle(api::TreeId, System);
impl LazyMemberHandle {
  pub fn run(self) -> OrcRes<MemberKind> {
    match self.1.get_tree(self.0) {
      api::MemberKind::Const(c) => Ok(MemberKind::PreCnst(RtExpr::from_api(c, &self.1))),
      api::MemberKind::Module(m) => Ok(MemberKind::Mod(Module::from_api(m, &self.1))),
      api::MemberKind::Lazy(id) => Self(id, self.1).run(),
    }
  }
}
