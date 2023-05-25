use std::ops::Add;
use std::rc::Rc;

use hashbrown::HashMap;

use crate::ast::{Expr, Rule};
use crate::interner::{Interner, Sym, Tok};
use crate::representations::tree::{ModMember, Module};
use crate::utils::Substack;

#[derive(Clone, Debug, Default)]
pub struct ProjectExt {
  /// Pairs each foreign token to the module it was imported from
  pub imports_from: HashMap<Tok<String>, Sym>,
  /// Pairs each exported token to its original full name.
  pub exports: HashMap<Tok<String>, Sym>,
  /// All rules defined in this module, exported or not
  pub rules: Vec<Rule>,
  /// Filename, if known, for error reporting
  pub file: Option<Vec<Tok<String>>>,
}

impl Add for ProjectExt {
  type Output = Self;

  fn add(mut self, rhs: Self) -> Self::Output {
    let ProjectExt { imports_from, exports, rules, file } = rhs;
    self.imports_from.extend(imports_from.into_iter());
    self.exports.extend(exports.into_iter());
    self.rules.extend(rules.into_iter());
    if file.is_some() {
      self.file = file
    }
    self
  }
}

pub type ProjectModule = Module<Expr, ProjectExt>;
pub struct ProjectTree(pub Rc<ProjectModule>);

fn collect_rules_rec(bag: &mut Vec<Rule>, module: &ProjectModule) {
  bag.extend(module.extra.rules.iter().cloned());
  for item in module.items.values() {
    if let ModMember::Sub(module) = &item.member {
      collect_rules_rec(bag, module.as_ref());
    }
  }
}

pub fn collect_rules(project: &ProjectTree) -> Vec<Rule> {
  let mut rules = Vec::new();
  collect_rules_rec(&mut rules, project.0.as_ref());
  rules
}

fn collect_consts_rec(
  path: Substack<Tok<String>>,
  bag: &mut HashMap<Sym, Expr>,
  module: &ProjectModule,
  i: &Interner,
) {
  for (key, entry) in module.items.iter() {
    match &entry.member {
      ModMember::Item(expr) => {
        let mut name = path.iter().rev_vec_clone();
        name.push(*key);
        bag.insert(i.i(&name), expr.clone());
      },
      ModMember::Sub(module) =>
        collect_consts_rec(path.push(*key), bag, module, i),
    }
  }
}

pub fn collect_consts(
  project: &ProjectTree,
  i: &Interner,
) -> HashMap<Sym, Expr> {
  let mut consts = HashMap::new();
  collect_consts_rec(Substack::Bottom, &mut consts, project.0.as_ref(), i);
  consts
}
