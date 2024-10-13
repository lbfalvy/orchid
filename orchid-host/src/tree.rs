use std::fmt::Debug;
use std::sync::{Mutex, OnceLock};

use itertools::Itertools;
use never::Never;
use orchid_base::error::OrcRes;
use orchid_base::interner::{deintern, intern, Tok};
use orchid_base::location::Pos;
use orchid_base::macros::{mtreev_from_api, MTree};
use orchid_base::name::Sym;
use orchid_base::parse::{Comment, Import};
use orchid_base::tree::{TokTree, Token};
use ordered_float::NotNan;
use substack::{with_iter_stack, Substack};

use crate::api;
use crate::expr::Expr;
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
  Member(Member),
  Export(Tok<String>),
  Import(Import),
  Macro(Option<NotNan<f64>>, Vec<Rule>)
}

impl Item {
  pub fn from_api<'a>(
    tree: api::Item,
    path: Substack<Tok<String>>,
    sys: &System
  ) -> Self {
    let kind = match tree.kind {
      api::ItemKind::Member(m) => ItemKind::Member(Member::from_api(m, path, sys)),
      api::ItemKind::Import(i) =>
        ItemKind::Import(Import{ path: Sym::deintern(i).iter().collect(), name: None }),
      api::ItemKind::Export(e) => ItemKind::Export(deintern(e)),
      api::ItemKind::Macro(api::MacroBlock { priority, rules }) => ItemKind::Macro(priority, {
        Vec::from_iter(rules.into_iter().map(|api| Rule {
          pos: Pos::from_api(&api.location),
          pattern: mtreev_from_api(&api.pattern),
          kind: RuleKind::Remote(sys.clone(), api.id),
          comments: api.comments.iter().map(Comment::from_api).collect_vec()
        }))
      })
    };
    let comments = tree.comments.iter().map(Comment::from_api).collect_vec();
    Self { pos: Pos::from_api(&tree.location), comments, kind }
  }
}

#[derive(Debug)]
pub struct Member {
  pub name: Tok<String>,
  pub kind: OnceLock<MemberKind>,
  pub lazy: Mutex<Option<LazyMemberHandle>>,
}
impl Member {
  pub fn from_api<'a>(
    api: api::Member,
    path: Substack<Tok<String>>,
    sys: &System,
  ) -> Self {
    let name = deintern(api.name);
    let full_path = path.push(name.clone());
    let kind = match api.kind {
      api::MemberKind::Lazy(id) =>
        return LazyMemberHandle(id, sys.clone(), intern(&full_path.unreverse())).to_member(name),
      api::MemberKind::Const(c) => MemberKind::Const(Code::from_expr(
        CodeLocator::to_const(full_path.unreverse()),
        Expr::from_api(c, &mut ())
      )),
      api::MemberKind::Module(m) => MemberKind::Mod(Module::from_api(m, full_path, sys)),
    };
    Member { name, kind: OnceLock::from(kind), lazy: Mutex::default() }
  }
  pub fn new(name: Tok<String>, kind: MemberKind) -> Self {
    Member { name, kind: OnceLock::from(kind), lazy: Mutex::default() }
  }
}

#[derive(Debug)]
pub enum MemberKind {
  Const(Code),
  Mod(Module),
}

#[derive(Debug)]
pub struct Module {
  pub imports: Vec<Sym>,
  pub exports: Vec<Tok<String>>,
  pub items: Vec<Item>,
}
impl Module {
  pub fn new(items: impl IntoIterator<Item = Item>) -> Self {
    let items = items.into_iter().collect_vec();
    let exports = (items.iter())
      .filter_map(|i| match &i.kind {
        ItemKind::Export(e) => Some(e.clone()),
        _ => None,
      })
      .collect_vec();
    Self { imports: vec![], exports, items }
  }
  pub fn from_api(m: api::Module, path: Substack<Tok<String>>, sys: &System) -> Self {
    let mut output = Vec::new();
    for item in m.items.into_iter() {
      let next = Item::from_api(item, path.clone(), sys);
      output.push(next);
    }
    Self::new(output)
  }
}

#[derive(Debug)]
pub struct LazyMemberHandle(api::TreeId, System, Tok<Vec<Tok<String>>>);
impl LazyMemberHandle {
  pub fn run(self) -> OrcRes<MemberKind> {
    match self.1.get_tree(self.0) {
      api::MemberKind::Const(c) => Ok(MemberKind::Const(Code {
        bytecode: Expr::from_api(c, &mut ()).into(),
        locator: CodeLocator { steps: self.2, rule_loc: None },
        source: None,
      })),
      api::MemberKind::Module(m) => with_iter_stack(self.2.iter().cloned(), |path| {
        Ok(MemberKind::Mod(Module::from_api(m, path, &self.1)))
      }),
      api::MemberKind::Lazy(id) => Self(id, self.1, self.2).run(),
    }
  }
  pub fn to_member(self, name: Tok<String>) -> Member {
    Member { name, kind: OnceLock::new(), lazy: Mutex::new(Some(self)) }
  }
}

#[derive(Debug)]
pub struct Rule {
  pub pos: Pos,
  pub comments: Vec<Comment>,
  pub pattern: Vec<MTree<'static>>,
  pub kind: RuleKind,
}

#[derive(Debug)]
pub enum RuleKind {
  Remote(System, api::MacroId),
  Native(Code),
}

#[derive(Debug)]
pub struct Code {
  locator: CodeLocator,
  source: Option<Vec<ParsTokTree>>,
  bytecode: OnceLock<Expr>,
}
impl Code {
  pub fn from_expr(locator: CodeLocator, expr: Expr) -> Self {
    Self { locator, source: None, bytecode: expr.into() }
  }
  pub fn from_code(locator: CodeLocator, code: Vec<ParsTokTree>) -> Self {
    Self { locator, source: Some(code), bytecode: OnceLock::new() }
  }
}

/// Selects a code element
/// 
/// Either the steps point to a constant and rule_loc is None, or the steps point to a module and
/// rule_loc selects a macro rule within that module
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct CodeLocator {
  steps: Tok<Vec<Tok<String>>>,
  /// Index of a macro block in the module demarked by the steps, and a rule in that macro
  rule_loc: Option<(u16, u16)>,
}
impl CodeLocator {
  pub fn to_const(path: impl IntoIterator<Item = Tok<String>>) -> Self {
    Self { steps: intern(&path.into_iter().collect_vec()), rule_loc: None }
  }
  pub fn to_rule(path: impl IntoIterator<Item = Tok<String>>, macro_i: u16, rule_i: u16) -> Self {
    Self { steps: intern(&path.into_iter().collect_vec()), rule_loc: Some((macro_i, rule_i)) }
  }
}