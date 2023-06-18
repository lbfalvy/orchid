use std::ops::Add;

use hashbrown::HashMap;

use crate::ast::{Expr, Rule};
use crate::interner::{Interner, Tok};
use crate::representations::tree::{ModMember, Module};
use crate::representations::NameLike;
use crate::tree::ModEntry;
use crate::utils::Substack;
use crate::{Sym, VName};

/// Additional data about a loaded module beyond the list of constants and
/// submodules
#[derive(Clone, Debug, Default)]
pub struct ProjectExt<N: NameLike> {
  /// Pairs each foreign token to the module it was imported from
  pub imports_from: HashMap<Tok<String>, N>,
  /// Pairs each exported token to its original full name
  pub exports: HashMap<Tok<String>, N>,
  /// All rules defined in this module, exported or not
  pub rules: Vec<Rule<N>>,
  /// Filename, if known, for error reporting
  pub file: Option<Vec<Tok<String>>>,
}

impl<N: NameLike> Add for ProjectExt<N> {
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

/// A node in the tree describing the project
pub type ProjectModule<N> = Module<Expr<N>, ProjectExt<N>>;

/// Module corresponding to the root of a project
#[derive(Debug, Clone)]
pub struct ProjectTree<N: NameLike>(pub ProjectModule<N>);

fn collect_rules_rec<N: NameLike>(
  bag: &mut Vec<Rule<N>>,
  module: &ProjectModule<N>,
) {
  bag.extend(module.extra.rules.iter().cloned());
  for item in module.items.values() {
    if let ModMember::Sub(module) = &item.member {
      collect_rules_rec(bag, module);
    }
  }
}

/// Collect the complete list of rules to be used by the rule repository from
/// the [ProjectTree]
pub fn collect_rules<N: NameLike>(project: &ProjectTree<N>) -> Vec<Rule<N>> {
  let mut rules = Vec::new();
  collect_rules_rec(&mut rules, &project.0);
  rules
}

fn collect_consts_rec<N: NameLike>(
  path: Substack<Tok<String>>,
  bag: &mut HashMap<Sym, Expr<N>>,
  module: &ProjectModule<N>,
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

/// Extract the symbol table from a [ProjectTree]
pub fn collect_consts<N: NameLike>(
  project: &ProjectTree<N>,
  i: &Interner,
) -> HashMap<Sym, Expr<N>> {
  let mut consts = HashMap::new();
  collect_consts_rec(Substack::Bottom, &mut consts, &project.0, i);
  consts
}

fn vname_to_sym_tree_rec(
  tree: ProjectModule<VName>,
  i: &Interner,
) -> ProjectModule<Sym> {
  let process_expr = |ex: Expr<VName>| ex.transform_names(&|n| i.i(&n));
  ProjectModule {
    imports: tree.imports,
    items: (tree.items.into_iter())
      .map(|(k, ModEntry { exported, member })| {
        (k, ModEntry {
          exported,
          member: match member {
            ModMember::Sub(module) =>
              ModMember::Sub(vname_to_sym_tree_rec(module, i)),
            ModMember::Item(ex) => ModMember::Item(process_expr(ex)),
          },
        })
      })
      .collect(),
    extra: ProjectExt {
      imports_from: (tree.extra.imports_from.into_iter())
        .map(|(k, v)| (k, i.i(&v)))
        .collect(),
      exports: (tree.extra.exports.into_iter())
        .map(|(k, v)| (k, i.i(&v)))
        .collect(),
      rules: (tree.extra.rules.into_iter())
        .map(|Rule { pattern, prio, template }| Rule {
          pattern: pattern.into_iter().map(process_expr).collect(),
          prio,
          template: template.into_iter().map(process_expr).collect(),
        })
        .collect(),
      file: tree.extra.file,
    },
  }
}

/// Convert a flexible vname-based tree to a more rigid but faster symbol-based
/// tree. The pipeline works with vnames, but the macro executor works with
/// symbols.
pub fn vname_to_sym_tree(
  tree: ProjectTree<VName>,
  i: &Interner,
) -> ProjectTree<Sym> {
  ProjectTree(vname_to_sym_tree_rec(tree.0, i))
}
