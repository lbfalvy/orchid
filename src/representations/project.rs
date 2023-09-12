use std::fmt::Display;
use std::ops::Add;

use hashbrown::HashMap;

use crate::ast::{Expr, Rule};
use crate::interner::{Interner, Tok};
use crate::representations::tree::{ModMember, Module};
use crate::representations::NameLike;
use crate::tree::ModEntry;
use crate::utils::never::{always, Always};
use crate::utils::Substack;
use crate::{Sym, VName};

#[derive(Debug, Clone)]
pub enum ItemKind<N: NameLike> {
  /// An imported symbol or module. The value is the absolute path of
  /// the symbol that should be used instead of this one.
  ///
  /// Notice that this is different from [ProjectExt::imports_from] the values
  /// of which do not include the name they're keyed with.
  Alias(VName),
  None,
  Const(Expr<N>),
}

impl<N: NameLike> Default for ItemKind<N> {
  fn default() -> Self {
    Self::None
  }
}

#[derive(Debug, Clone, Default)]
pub struct ProjectItem<N: NameLike> {
  pub kind: ItemKind<N>,
  pub is_op: bool,
}

impl<N: NameLike> Display for ProjectItem<N> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match &self.kind {
      ItemKind::None => match self.is_op {
        true => write!(f, "operator"),
        false => write!(f, "keyword"),
      },
      ItemKind::Const(c) => match self.is_op {
        true => write!(f, "operator with value {c}"),
        false => write!(f, "constant {c}"),
      },
      ItemKind::Alias(alias) => {
        let origin = Interner::extern_all(alias).join("::");
        match self.is_op {
          true => write!(f, "operator alias to {origin}"),
          false => write!(f, "alias to {origin}"),
        }
      },
    }
  }
}

/// Information about an imported symbol
#[derive(Debug, Clone)]
pub struct ImpReport<N: NameLike> {
  /// Absolute path of the module the symbol is imported from
  pub source: N,
  /// Whether this symbol should be treated as an operator for the purpose of
  /// parsing
  pub is_op: bool,
}

/// Additional data about a loaded module beyond the list of constants and
/// submodules
#[derive(Clone, Debug)]
pub struct ProjectExt<N: NameLike> {
  /// Full path leading to this module
  pub path: VName,
  /// Pairs each imported token to the absolute path of the module it is
  /// imported from. The path does not include the name of referencedthe
  /// symbol.
  pub imports_from: HashMap<Tok<String>, ImpReport<N>>,
  /// All rules defined in this module, exported or not
  pub rules: Vec<Rule<N>>,
  /// Filename, if known, for error reporting
  pub file: Option<VName>,
}

impl<N: NameLike> Add for ProjectExt<N> {
  type Output = Always<Self>;

  fn add(mut self, rhs: Self) -> Self::Output {
    let ProjectExt { path, imports_from, rules, file } = rhs;
    if path != self.path {
      panic!(
        "Differently named trees overlain: {} vs {}",
        Interner::extern_all(&path).join("::"),
        Interner::extern_all(&self.path).join("::")
      )
    }
    self.imports_from.extend(imports_from.into_iter());
    self.rules.extend(rules.into_iter());
    if file.is_some() {
      self.file = file
    }
    always(self)
  }
}

impl<N: NameLike> Display for ProjectExt<N> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Project-module extras")
      .field("path", &Interner::extern_all(&self.path).join("::"))
      .field("imports_from", &self.imports_from)
      .field("rules", &self.rules)
      .field("file", &(Interner::extern_all(&self.path).join("/") + ".orc"))
      .finish()
  }
}

/// A child to a [ProjectMod]
pub type ProjectEntry<N> = ModEntry<ProjectItem<N>, ProjectExt<N>>;
/// A node in the tree describing the project
pub type ProjectMod<N> = Module<ProjectItem<N>, ProjectExt<N>>;

/// Module corresponding to the root of a project
#[derive(Debug, Clone)]
pub struct ProjectTree<N: NameLike>(pub ProjectMod<N>);

fn collect_rules_rec<N: NameLike>(
  bag: &mut Vec<Rule<N>>,
  module: &ProjectMod<N>,
) {
  bag.extend(module.extra.rules.iter().cloned());
  for item in module.entries.values() {
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
  module: &ProjectMod<N>,
  i: &Interner,
) {
  for (key, entry) in module.entries.iter() {
    match &entry.member {
      ModMember::Item(it) =>
        if let ItemKind::Const(expr) = &it.kind {
          let mut name = path.iter().rev_vec_clone();
          name.push(key.clone());
          bag.insert(i.i(&name), expr.clone());
        },
      ModMember::Sub(module) =>
        collect_consts_rec(path.push(key.clone()), bag, module, i),
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
  tree: ProjectMod<VName>,
  i: &Interner,
) -> ProjectMod<Sym> {
  let process_expr = |ex: Expr<VName>| ex.transform_names(&|n| i.i(&n));
  ProjectMod {
    entries: (tree.entries.into_iter())
      .map(|(k, ModEntry { exported, member })| {
        (k, ModEntry {
          exported,
          member: match member {
            ModMember::Sub(module) =>
              ModMember::Sub(vname_to_sym_tree_rec(module, i)),
            ModMember::Item(ex) => ModMember::Item(ProjectItem {
              is_op: ex.is_op,
              kind: match ex.kind {
                ItemKind::None => ItemKind::None,
                ItemKind::Alias(n) => ItemKind::Alias(n),
                ItemKind::Const(ex) => ItemKind::Const(process_expr(ex)),
              },
            }),
          },
        })
      })
      .collect(),
    extra: ProjectExt {
      path: tree.extra.path,
      imports_from: (tree.extra.imports_from.into_iter())
        .map(|(k, v)| (k, ImpReport { is_op: v.is_op, source: i.i(&v.source) }))
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
