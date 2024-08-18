use std::num::NonZero;
use std::ops::Range;
use std::sync::Arc;

use dyn_clone::{clone_box, DynClone};
use hashbrown::HashMap;
use itertools::Itertools;
use orchid_base::interner::{intern, Tok};
use orchid_base::location::Pos;
use orchid_base::name::Sym;
use orchid_base::tree::{ttv_to_api, TokTree, Token};
use ordered_float::NotNan;
use substack::Substack;
use trait_set::trait_set;

use crate::api;
use crate::atom::{AtomFactory, ForeignAtom};
use crate::conv::ToExpr;
use crate::entrypoint::MemberRecord;
use crate::expr::GenExpr;
use crate::func_atom::{ExprFunc, Fun};
use crate::system::SysCtx;

pub type GenTokTree<'a> = TokTree<'a, ForeignAtom<'a>, AtomFactory>;
pub type GenTok<'a> = Token<'a, ForeignAtom<'a>, AtomFactory>;

pub fn do_extra(f: &AtomFactory, r: Range<u32>, ctx: SysCtx) -> api::TokenTree {
  api::TokenTree { range: r, token: api::Token::Atom(f.clone().build(ctx)) }
}

#[derive(Clone)]
pub struct GenMacro {
  pub pattern: Vec<GenTokTree<'static>>,
  pub priority: NotNan<f64>,
  pub template: Vec<GenTokTree<'static>>,
}

pub struct GenItem {
  pub item: GenItemKind,
  pub comments: Vec<(String, Pos)>,
  pub pos: Pos,
}
impl GenItem {
  pub fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> api::Item {
    let kind = match self.item {
      GenItemKind::Rule(m) => api::ItemKind::Rule(api::Macro {
        pattern: ttv_to_api(m.pattern, &mut |f, r| do_extra(f, r, ctx.sys())),
        priority: m.priority,
        template: ttv_to_api(m.template, &mut |f, r| do_extra(f, r, ctx.sys())),
      }),
      GenItemKind::Raw(item) => api::ItemKind::Raw(Vec::from_iter(
        item.into_iter().map(|t| t.to_api(&mut |f, r| do_extra(f, r, ctx.sys()))),
      )),
      GenItemKind::Member(mem) => api::ItemKind::Member(mem.into_api(ctx)),
    };
    let comments = self.comments.into_iter().map(|(s, p)| (Arc::new(s), p.to_api())).collect_vec();
    api::Item { location: self.pos.to_api(), comments, kind }
  }
}

pub fn cnst(public: bool, name: &str, value: impl ToExpr) -> GenItem {
  let kind = GenMemberKind::Const(value.to_expr());
  GenItemKind::Member(GenMember { exported: public, name: intern(name), kind }).at(Pos::Inherit)
}
pub fn module(
  public: bool,
  name: &str,
  imports: impl IntoIterator<Item = Sym>,
  items: impl IntoIterator<Item = GenItem>,
) -> GenItem {
  let (name, kind) = root_mod(name, imports, items);
  GenItemKind::Member(GenMember { exported: public, name, kind }).at(Pos::Inherit)
}
pub fn root_mod(
  name: &str,
  imports: impl IntoIterator<Item = Sym>,
  items: impl IntoIterator<Item = GenItem>,
) -> (Tok<String>, GenMemberKind) {
  let kind = GenMemberKind::Mod {
    imports: imports.into_iter().collect(),
    items: items.into_iter().collect(),
  };
  (intern(name), kind)
}
pub fn fun<I, O>(exported: bool, name: &str, xf: impl ExprFunc<I, O>) -> GenItem {
  let fac = LazyMemberFactory::new(move |sym| GenMemberKind::Const(Fun::new(sym, xf).to_expr()));
  let mem = GenMember{ exported, name: intern(name), kind: GenMemberKind::Lazy(fac) };
  GenItemKind::Member(mem).at(Pos::Inherit)
}
pub fn rule(
  priority: f64,
  pat: impl IntoIterator<Item = GenTokTree<'static>>,
  tpl: impl IntoIterator<Item = GenTokTree<'static>>,
) -> GenItem {
  GenItemKind::Rule(GenMacro {
    pattern: pat.into_iter().collect(),
    priority: NotNan::new(priority).expect("Rule created with NaN prio"),
    template: tpl.into_iter().collect(),
  })
  .at(Pos::Inherit)
}

pub fn comments<'a>(cmts: impl IntoIterator<Item = &'a str>, mut val: GenItem) -> GenItem {
  val.comments.extend(cmts.into_iter().map(|c| (c.to_string(), Pos::Inherit)));
  val
}

trait_set! {
  trait LazyMemberCallback = FnOnce(Sym) -> GenMemberKind + Send + Sync + DynClone
}
pub struct LazyMemberFactory(Box<dyn LazyMemberCallback>);
impl LazyMemberFactory {
  pub fn new(cb: impl FnOnce(Sym) -> GenMemberKind + Send + Sync + Clone + 'static) -> Self {
    Self(Box::new(cb))
  }
  pub fn build(self, path: Sym) -> GenMemberKind { (self.0)(path) }
}
impl Clone for LazyMemberFactory {
  fn clone(&self) -> Self { Self(clone_box(&*self.0)) }
}

pub enum GenItemKind {
  Member(GenMember),
  Raw(Vec<GenTokTree<'static>>),
  Rule(GenMacro),
}
impl GenItemKind {
  pub fn at(self, position: Pos) -> GenItem {
    GenItem { item: self, comments: vec![], pos: position }
  }
}

pub struct GenMember {
  exported: bool,
  name: Tok<String>,
  kind: GenMemberKind,
}
impl GenMember {
  pub fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> api::Member {
    api::Member {
      name: self.name.marker(),
      exported: self.exported,
      kind: self.kind.into_api(&mut ctx.push_path(self.name))
    }
  }
}

pub enum GenMemberKind {
  Const(GenExpr),
  Mod { imports: Vec<Sym>, items: Vec<GenItem> },
  Lazy(LazyMemberFactory),
}
impl GenMemberKind {
  pub fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> api::MemberKind {
    match self {
      Self::Lazy(lazy) => api::MemberKind::Lazy(ctx.with_lazy(lazy)),
      Self::Const(c) => api::MemberKind::Const(c.into_api(ctx.sys())),
      Self::Mod { imports, items } => api::MemberKind::Module(api::Module {
        imports: imports.into_iter().map(|t| t.tok().marker()).collect(),
        items: items.into_iter().map(|i| i.into_api(ctx)).collect_vec(),
      }),
    }
  }
}

pub trait TreeIntoApiCtx {
  fn sys(&self) -> SysCtx;
  fn with_lazy(&mut self, fac: LazyMemberFactory) -> api::TreeId;
  fn push_path(&mut self, seg: Tok<String>) -> impl TreeIntoApiCtx;
}

pub struct TIACtxImpl<'a, 'b> {
  pub ctx: SysCtx,
  pub basepath: &'a [Tok<String>],
  pub path: Substack<'a, Tok<String>>,
  pub lazy: &'b mut HashMap<api::TreeId, MemberRecord>,
}

impl<'a, 'b> TreeIntoApiCtx for TIACtxImpl<'a, 'b> {
  fn sys(&self) -> SysCtx { self.ctx.clone() }
  fn push_path(&mut self, seg: Tok<String>) -> impl TreeIntoApiCtx {
    TIACtxImpl {
      ctx: self.ctx.clone(),
      lazy: self.lazy,
      basepath: self.basepath,
      path: self.path.push(seg)
    }
  }
  fn with_lazy(&mut self, fac: LazyMemberFactory) -> api::TreeId {
    let id = api::TreeId(NonZero::new((self.lazy.len() + 2) as u64).unwrap());
    let path = Sym::new(self.basepath.iter().cloned().chain(self.path.unreverse())).unwrap();
    self.lazy.insert(id, MemberRecord::Gen(path, fac));
    id
  }
}
