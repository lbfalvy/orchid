use ahash::HashMap;
use lazy_static::lazy_static;
use orchid_base::{error::OrcRes, interner::{intern, Tok}, location::Pos, macros::{mtreev_from_api, mtreev_to_api, MTree}, parse::Comment, reqnot::Requester};
use trait_set::trait_set;
use crate::{api, lexer::err_cascade, system::SysCtx};
use std::{num::NonZero, sync::RwLock};

pub trait Macro {
  fn pattern() -> MTree<'static>;
  fn apply(binds: HashMap<Tok<String>, MTree<'_>>) -> MTree<'_>;
}

pub trait DynMacro {
  fn pattern(&self) -> MTree<'static>;
  fn apply<'a>(&self, binds: HashMap<Tok<String>, MTree<'a>>) -> MTree<'a>;
}

impl<T: Macro> DynMacro for T {
  fn pattern(&self) -> MTree<'static> { Self::pattern() }
  fn apply<'a>(&self, binds: HashMap<Tok<String>, MTree<'a>>) -> MTree<'a> { Self::apply(binds) }
}

pub struct RuleCtx<'a> {
  pub(crate) args: HashMap<Tok<String>, Vec<MTree<'a>>>,
  pub(crate) run_id: api::ParsId,
  pub(crate) sys: SysCtx,
}
impl<'a> RuleCtx<'a> {
  pub fn recurse(&mut self, tree: &[MTree<'a>]) -> OrcRes<Vec<MTree<'a>>> {
    let req = api::RunMacros{ run_id: self.run_id, query: mtreev_to_api(tree) };
    Ok(mtreev_from_api(&self.sys.reqnot.request(req).ok_or_else(err_cascade)?))
  }
  pub fn getv(&mut self, key: &Tok<String>) -> Vec<MTree<'a>> {
    self.args.remove(key).expect("Key not found")
  }
  pub fn gets(&mut self, key: &Tok<String>) -> MTree<'a> {
    let v = self.getv(key);
    assert!(v.len() == 1, "Not a scalar");
    v.into_iter().next().unwrap()
  }
  pub fn unused_arg<'b>(&mut self, keys: impl IntoIterator<Item = &'b Tok<String>>) {
    keys.into_iter().for_each(|k| {self.getv(k);});
  }
}

trait_set! {
  pub trait RuleCB = for<'a> Fn(RuleCtx<'a>) -> OrcRes<Vec<MTree<'a>>> + Send + Sync;
}

lazy_static!{
  static ref RULES: RwLock<HashMap<api::MacroId, Box<dyn RuleCB>>> = RwLock::default();
}

pub struct Rule {
  pub(crate) comments: Vec<Comment>,
  pub(crate) pattern: Vec<MTree<'static>>,
  pub(crate) id: api::MacroId,
}
impl Rule {
  pub(crate) fn to_api(&self) -> api::MacroRule {
    api::MacroRule {
      comments: self.comments.iter().map(|c| c.to_api()).collect(),
      location: api::Location::Inherit,
      pattern: mtreev_to_api(&self.pattern),
      id: self.id,
    }
  }
}

pub fn rule_cmt<'a>(
  cmt: impl IntoIterator<Item = &'a str>,
  pattern: Vec<MTree<'static>>,
  apply: impl RuleCB + 'static
) -> Rule {
  let mut rules = RULES.write().unwrap();
  let id = api::MacroId(NonZero::new(rules.len() as u64 + 1).unwrap());
  rules.insert(id, Box::new(apply));
  let comments = cmt.into_iter().map(|s| Comment { pos: Pos::Inherit, text: intern(s) }).collect();
  Rule { comments, pattern, id }
}

pub fn rule(pattern: Vec<MTree<'static>>, apply: impl RuleCB + 'static) -> Rule {
  rule_cmt([], pattern, apply)
}

pub(crate) fn apply_rule(id: api::MacroId, ctx: RuleCtx<'static>) -> OrcRes<Vec<MTree<'static>>> {
  let rules = RULES.read().unwrap();
  rules[&id](ctx)
}