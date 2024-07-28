use std::iter;
use std::num::NonZero;
use std::ops::Range;

use hashbrown::HashMap;
use dyn_clone::{clone_box, DynClone};
use itertools::Itertools;
use orchid_api::tree::{
  Macro, Paren, PlaceholderKind, Token, TokenTree, Item, TreeId, ItemKind, Member, MemberKind, Module, TreeTicket
};
use orchid_base::interner::{intern, Tok};
use orchid_base::location::Pos;
use orchid_base::name::{NameLike, Sym, VName};
use orchid_base::tokens::OwnedPh;
use ordered_float::NotNan;
use trait_set::trait_set;

use crate::atom::AtomFactory;
use crate::conv::ToExpr;
use crate::entrypoint::MemberRecord;
use crate::error::{errv_to_apiv, ProjectErrorObj};
use crate::expr::GenExpr;
use crate::system::DynSystem;

#[derive(Clone)]
pub struct GenTokTree {
  pub tok: GenTok,
  pub range: Range<u32>,
}
impl GenTokTree {
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
pub enum GenTok {
  Lambda(Vec<GenTokTree>),
  Name(Tok<String>),
  NS,
  BR,
  S(Paren, Vec<GenTokTree>),
  Atom(AtomFactory),
  Slot(TreeTicket),
  Ph(OwnedPh),
  Bottom(ProjectErrorObj),
}
impl GenTok {
  pub fn at(self, range: Range<u32>) -> GenTokTree { GenTokTree { tok: self, range } }
  pub fn into_api(self, sys: &dyn DynSystem) -> Token {
    match self {
      Self::Lambda(x) => Token::Lambda(x.into_iter().map(|tt| tt.into_api(sys)).collect()),
      Self::Name(n) => Token::Name(n.marker()),
      Self::NS => Token::NS,
      Self::BR => Token::BR,
      Self::Ph(ph) => Token::Ph(ph.to_api()),
      Self::S(p, body) => Token::S(p, body.into_iter().map(|tt| tt.into_api(sys)).collect_vec()),
      Self::Slot(tk) => Token::Slot(tk),
      Self::Atom(at) => Token::Atom(at.build(sys)),
      Self::Bottom(err) => Token::Bottom(errv_to_apiv([err])),
    }
  }
  pub fn vname(name: &VName) -> impl Iterator<Item = GenTok> + '_ {
    let (head, tail) = name.split_first();
    iter::once(Self::Name(head)).chain(tail.iter().flat_map(|t| [Self::NS, Self::Name(t)]))
  }
}

#[derive(Clone)]
pub struct GenMacro {
  pub pattern: Vec<GenTokTree>,
  pub priority: NotNan<f64>,
  pub template: Vec<GenTokTree>,
}

pub fn tokv_into_api(
  tokv: impl IntoIterator<Item = GenTokTree>,
  sys: &dyn DynSystem,
) -> Vec<TokenTree> {
  tokv.into_iter().map(|tok| tok.into_api(sys)).collect_vec()
}

pub fn wrap_tokv(items: Vec<GenTokTree>, range: Range<u32>) -> GenTokTree {
  match items.len() {
    1 => items.into_iter().next().unwrap(),
    _ => GenTok::S(Paren::Round, items).at(range),
  }
}

pub struct GenItem {
  pub item: GenItemKind,
  pub pos: Pos,
}
impl GenItem {
  pub fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> Item {
    let kind = match self.item {
      GenItemKind::Rule(GenMacro { pattern, priority, template }) => ItemKind::Rule(Macro {
        pattern: tokv_into_api(pattern, ctx.sys()),
        priority,
        template: tokv_into_api(template, ctx.sys()),
      }),
      GenItemKind::Raw(item) => ItemKind::Raw(item.into_iter().map(|t| t.into_api(ctx.sys())).collect_vec()),
      GenItemKind::Member(mem) => ItemKind::Member(mem.into_api(ctx))
    };
    Item { location: self.pos.to_api(), kind }
  }
}

pub fn cnst(public: bool, name: &str, value: impl ToExpr) -> GenItem {
  let kind = GenMemberKind::Const(value.to_expr());
  GenItemKind::Member(GenMember { public, name: intern(name), kind }).at(Pos::Inherit)
}
pub fn module(
  public: bool,
  name: &str,
  imports: impl IntoIterator<Item = Sym>,
  items: impl IntoIterator<Item = GenItem>
) -> GenItem {
  let (name, kind) = root_mod(name, imports, items);
  GenItemKind::Member(GenMember { public, name, kind }).at(Pos::Inherit)
}
pub fn root_mod(
  name: &str, 
  imports: impl IntoIterator<Item = Sym>,
  items: impl IntoIterator<Item = GenItem>
) -> (Tok<String>, GenMemberKind) {
  let kind = GenMemberKind::Mod {
    imports: imports.into_iter().collect(),
    items: items.into_iter().collect()
  };
  (intern(name), kind)
}
pub fn rule(
  prio: f64,
  pat: impl IntoIterator<Item = GenTokTree>,
  tpl: impl IntoIterator<Item = GenTokTree>,
) -> GenItem {
  GenItemKind::Rule(GenMacro {
    pattern: pat.into_iter().collect(),
    priority: NotNan::new(prio).expect("expected to be static"),
    template: tpl.into_iter().collect(),
  })
  .at(Pos::Inherit)
}

trait_set! {
  trait LazyMemberCallback = FnOnce() -> GenMemberKind + Send + Sync + DynClone
}
pub struct LazyMemberFactory(Box<dyn LazyMemberCallback>);
impl LazyMemberFactory {
  pub fn new(cb: impl FnOnce() -> GenMemberKind + Send + Sync + Clone + 'static) -> Self {
    Self(Box::new(cb))
  }
  pub fn build(self) -> GenMemberKind { (self.0)() }
}
impl Clone for LazyMemberFactory {
  fn clone(&self) -> Self { Self(clone_box(&*self.0)) }
}

pub enum GenItemKind {
  Member(GenMember),
  Raw(Vec<GenTokTree>),
  Rule(GenMacro),
}
impl GenItemKind {
  pub fn at(self, position: Pos) -> GenItem { GenItem { item: self, pos: position } }
}

pub struct GenMember {
  public: bool,
  name: Tok<String>,
  kind: GenMemberKind,
}
impl GenMember {
  pub fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> Member {
    Member { name: self.name.marker(), public: self.public, kind: self.kind.into_api(ctx) }
  }
}

pub enum GenMemberKind {
  Const(GenExpr),
  Mod{
    imports: Vec<Sym>,
    items: Vec<GenItem>,
  },
  Lazy(LazyMemberFactory)
}
impl GenMemberKind {
  pub fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> MemberKind {
    match self {
      Self::Lazy(lazy) => MemberKind::Lazy(ctx.with_lazy(lazy)),
      Self::Const(c) => MemberKind::Const(c.into_api(ctx.sys())),
      Self::Mod { imports, items } => MemberKind::Module(Module {
        imports: imports.into_iter().map(|t| t.tok().marker()).collect(),
        items: items.into_iter().map(|i| i.into_api(ctx)).collect_vec()
      }),
    }
  }
}

pub trait TreeIntoApiCtx {
  fn sys(&self) -> &dyn DynSystem;
  fn with_lazy(&mut self, fac: LazyMemberFactory) -> TreeId;
}

pub struct TIACtxImpl<'a> {
  pub sys: &'a dyn DynSystem,
  pub lazy: &'a mut HashMap<TreeId, MemberRecord>
}

impl<'a> TreeIntoApiCtx for TIACtxImpl<'a> {
  fn sys(&self) -> &dyn DynSystem { self.sys }
  fn with_lazy(&mut self, fac: LazyMemberFactory) -> TreeId {
    let id = TreeId(NonZero::new((self.lazy.len() + 2) as u64).unwrap());
    self.lazy.insert(id, MemberRecord::Gen(fac));
    id
  }
}
