use std::borrow::Borrow;
use std::cell::RefCell;
use std::fmt::{Display, Write};
use std::ops::Range;
use std::sync::{Mutex, OnceLock};

use itertools::Itertools;
use never::Never;
use orchid_api::tree::{Item, ItemKind, Macro, Member, MemberKind, Module, Paren, Token, TokenTree, TreeId, TreeTicket};
use orchid_base::error::OwnedError;
use orchid_base::interner::{deintern, Tok};
use orchid_base::location::Pos;
use orchid_base::name::Sym;
use orchid_base::tokens::{OwnedPh, PARENS};
use ordered_float::NotNan;

use crate::expr::RtExpr;
use crate::extension::{AtomHand, System};
use crate::results::OwnedResult;

#[derive(Clone, Debug)]
pub struct OwnedTokTree {
  pub tok: OwnedTok,
  pub range: Range<u32>,
}
impl OwnedTokTree {
  pub fn from_api<E>(
    tt: &TokenTree,
    sys: &System,
    do_slot: &mut impl FnMut(&TreeTicket, Range<u32>) -> Result<Self, E>,
  ) -> Result<Self, E> {
    let tok = match &tt.token {
      Token::Atom(a) => OwnedTok::Atom(AtomHand::from_api(a.clone().associate(sys.id()))),
      Token::BR => OwnedTok::BR,
      Token::NS => OwnedTok::NS,
      Token::Bottom(e) => OwnedTok::Bottom(e.iter().map(OwnedError::from_api).collect_vec()),
      Token::Lambda(arg) => OwnedTok::Lambda(Self::v_from_api(arg, sys, do_slot)?),
      Token::Name(name) => OwnedTok::Name(deintern(*name)),
      Token::Ph(ph) => OwnedTok::Ph(OwnedPh::from_api(ph.clone())),
      Token::S(par, b) => OwnedTok::S(par.clone(), Self::v_from_api(b, sys, do_slot)?),
      Token::Slot(id) => return do_slot(id, tt.range.clone()),
    };
    Ok(Self { range: tt.range.clone(), tok })
  }

  pub fn v_from_api<E>(
    tokv: impl IntoIterator<Item: Borrow<TokenTree>>,
    sys: &System,
    do_slot: &mut impl FnMut(&TreeTicket, Range<u32>) -> Result<Self, E>,
  ) -> Result<Vec<Self>, E> {
    tokv.into_iter().map(|t| Self::from_api(t.borrow(), sys, do_slot)).collect()
  }
}
impl Display for OwnedTokTree {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.tok) }
}

#[derive(Clone, Debug)]
pub enum OwnedTok {
  Comment(String),
  Lambda(Vec<OwnedTokTree>),
  Name(Tok<String>),
  NS,
  BR,
  S(Paren, Vec<OwnedTokTree>),
  Atom(AtomHand),
  Ph(OwnedPh),
  Bottom(Vec<OwnedError>),
}
impl Display for OwnedTok {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    thread_local! {
      static PAREN_LEVEL: RefCell<usize> = 0.into();
    }
    fn get_indent() -> usize { PAREN_LEVEL.with_borrow(|t| *t) }
    fn with_indent<T>(f: impl FnOnce() -> T) -> T {
      PAREN_LEVEL.with_borrow_mut(|t| *t += 1);
      let r = f();
      PAREN_LEVEL.with_borrow_mut(|t| *t -= 1);
      r
    }
    match self {
      Self::Atom(ah) => f.write_str(&indent(&ah.print(), get_indent(), false)),
      Self::BR => write!(f, "\n{}", "  ".repeat(get_indent())),
      Self::Bottom(err) => write!(f, "Botttom({})",
        err.iter().map(|e| format!("{}: {}", e.description, e.message)).join(", ")
      ),
      Self::Comment(c) => write!(f, "--[{c}]--"),
      Self::Lambda(arg) => with_indent(|| write!(f, "\\ {} .", fmt_tt_v(arg))),
      Self::NS => f.write_str("::"),
      Self::Name(n) => f.write_str(n),
      Self::Ph(ph) => write!(f, "{ph}"),
      Self::S(p, b) => {
        let (lp, rp, _) = PARENS.iter().find(|(_, _, par)| par == p).unwrap();
        f.write_char(*lp)?;
        with_indent(|| f.write_str(&fmt_tt_v(b)))?;
        f.write_char(*rp)
      }
    }
  }
}

pub fn fmt_tt_v<'a>(ttv: impl IntoIterator<Item = &'a OwnedTokTree>) -> String {
  ttv.into_iter().join(" ")
}

pub fn indent(s: &str, lvl: usize, first: bool) -> String {
  if first {
    s.replace("\n", &("\n".to_string() + &"  ".repeat(lvl)))
  } else if let Some((fst, rest)) = s.split_once('\n') {
    fst.to_string() + "\n" + &indent(rest, lvl, true)
  } else {
    s.to_string()
  }
}

#[derive(Debug)]
pub struct OwnedItem {
  pub pos: Pos,
  pub kind: OwnedItemKind,
}

#[derive(Debug)]
pub enum OwnedItemKind {
  Raw(Vec<OwnedTokTree>),
  Member(OwnedMember),
  Rule(OwnedMacro),
}

fn slot_panic(_: &TreeTicket, _: Range<u32>) -> Result<OwnedTokTree, Never> {
  panic!("No slots allowed at item stage")
}

impl OwnedItem {
  pub fn from_api(tree: Item, sys: &System) -> Self {
    let kind = match tree.kind {
      ItemKind::Raw(tokv) =>
        OwnedItemKind::Raw(OwnedTokTree::v_from_api::<Never>(tokv, sys, &mut slot_panic).unwrap()),
      ItemKind::Member(m) => OwnedItemKind::Member(OwnedMember::from_api(m, sys)),
      ItemKind::Rule(r) => OwnedItemKind::Rule(OwnedMacro::from_api(r, sys)),
    };
    Self { pos: Pos::from_api(&tree.location), kind }
  }
}

#[derive(Debug)]
pub struct OwnedMember {
  pub public: bool,
  pub name: Tok<String>,
  pub kind: OnceLock<OMemKind>,
  pub lazy: Mutex<Option<LazyMemberHandle>>,
}
impl OwnedMember {
  pub fn from_api(Member{ public, name, kind }: Member, sys: &System) -> Self {
    let (kind, lazy) = match kind {
      MemberKind::Const(c) => (OnceLock::from(OMemKind::Const(RtExpr::from_api(c, sys))), None),
      MemberKind::Module(m) => (OnceLock::from(OMemKind::Mod(OwnedModule::from_api(m, sys))), None),
      MemberKind::Lazy(id) => (OnceLock::new(), Some(LazyMemberHandle(id, sys.clone())))
    };
    OwnedMember { public, name: deintern(name), kind, lazy: Mutex::new(lazy) }
  }
}

#[derive(Debug)]
pub enum OMemKind {
  Const(RtExpr),
  Mod(OwnedModule),
}

#[derive(Debug)]
pub struct OwnedModule {
  pub imports: Vec<Sym>,
  pub items: Vec<OwnedItem>,
}
impl OwnedModule {
  pub fn from_api(m: Module, sys: &System) -> Self {
    Self {
      imports: m.imports.into_iter().map(|m| Sym::from_tok(deintern(m)).unwrap()).collect_vec(),
      items: m.items.into_iter().map(|i| OwnedItem::from_api(i, sys)).collect_vec(),
    }
  }
}

#[derive(Debug)]
pub struct OwnedMacro {
  pub priority: NotNan<f64>,
  pub pattern: Vec<OwnedTokTree>,
  pub template: Vec<OwnedTokTree>,
}
impl OwnedMacro {
  pub fn from_api(m: Macro, sys: &System) -> Self {
    Self {
      priority: m.priority,
      pattern: OwnedTokTree::v_from_api(m.pattern, sys, &mut slot_panic).unwrap(),
      template: OwnedTokTree::v_from_api(m.template, sys, &mut slot_panic).unwrap(),
    }
  }
}

#[derive(Debug)]
pub struct LazyMemberHandle(TreeId, System);
impl LazyMemberHandle {
  pub fn run(self) -> OwnedResult<OMemKind> {
    match self.1.get_tree(self.0) {
      MemberKind::Const(c) => Ok(OMemKind::Const(RtExpr::from_api(c, &self.1))),
      MemberKind::Module(m) => Ok(OMemKind::Mod(OwnedModule::from_api(m, &self.1))),
      MemberKind::Lazy(id) => Self(id, self.1).run()
    }
  }
}