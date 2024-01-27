use std::fmt::{self, Display};

use hashbrown::HashSet;
use intern_all::Tok;

use crate::error::{ErrorPosition, ProjectError, ProjectErrorObj};
use crate::location::{CodeLocation, SourceRange};
use crate::parse::parsed::{search_all_slcs, Clause, PHClass, Placeholder};
use crate::pipeline::project::ProjRule;
use crate::utils::boxed_iter::BoxedIter;

/// Various reasons why a substitution rule may be invalid
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {
  /// A key is present in the template but not the pattern
  Missing(Tok<String>),
  /// A key uses a different arity in the template and in the pattern
  ArityMismatch(Tok<String>),
  /// Multiple occurences of a placeholder in a pattern
  Multiple(Tok<String>),
  /// Two vectorial placeholders are next to each other
  VecNeighbors(Tok<String>, Tok<String>),
}
impl RuleError {
  /// Convert into a unified error trait object shared by all Orchid errors
  #[must_use]
  pub fn to_project(self, rule: &ProjRule) -> ProjectErrorObj {
    match self {
      Self::Missing(name) => Missing::new(rule, name).pack(),
      Self::Multiple(name) => Multiple::new(rule, name).pack(),
      Self::ArityMismatch(name) => ArityMismatch::new(rule, name).pack(),
      Self::VecNeighbors(n1, n2) => VecNeighbors::new(rule, n1, n2).pack(),
    }
  }
}

impl Display for RuleError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Missing(key) => write!(f, "Key {key} not in match pattern"),
      Self::ArityMismatch(key) => {
        write!(f, "Key {key} used inconsistently with and without ellipsis")
      },
      Self::Multiple(key) => {
        write!(f, "Key {key} appears multiple times in match pattern")
      },
      Self::VecNeighbors(left, right) => {
        write!(f, "vectorials {left} and {right} are next to each other")
      },
    }
  }
}

/// A key is present in the template but not the pattern of a rule
#[derive(Debug)]
struct Missing {
  locations: HashSet<SourceRange>,
  name: Tok<String>,
}
impl Missing {
  #[must_use]
  pub fn new(rule: &ProjRule, name: Tok<String>) -> Self {
    let mut locations = HashSet::new();
    for expr in rule.template.iter() {
      expr.search_all(&mut |e| {
        if let Clause::Placeh(ph) = &e.value {
          if ph.name == name {
            locations.insert(e.range.clone());
          }
        }
        None::<()>
      });
    }
    Self { locations, name }
  }
}
impl ProjectError for Missing {
  const DESCRIPTION: &'static str =
    "A key appears in the template but not the pattern of a rule";
  fn message(&self) -> String {
    format!(
      "The key {} appears in the template but not the pattern of this rule",
      self.name
    )
  }
  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> {
    (self.locations.iter()).cloned().map(|range| ErrorPosition {
      location: CodeLocation::Source(range),
      message: None,
    })
  }
}

/// A key is present multiple times in the pattern of a rule
#[derive(Debug)]
struct Multiple {
  locations: HashSet<SourceRange>,
  name: Tok<String>,
}
impl Multiple {
  #[must_use]
  pub fn new(rule: &ProjRule, name: Tok<String>) -> Self {
    let mut locations = HashSet::new();
    for expr in rule.template.iter() {
      expr.search_all(&mut |e| {
        if let Clause::Placeh(ph) = &e.value {
          if ph.name == name {
            locations.insert(e.range.clone());
          }
        }
        None::<()>
      });
    }
    Self { locations, name }
  }
}
impl ProjectError for Multiple {
  const DESCRIPTION: &'static str =
    "A key appears multiple times in the pattern of a rule";
  fn message(&self) -> String {
    format!("The key {} appears multiple times in this pattern", self.name)
  }
  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> {
    (self.locations.iter()).cloned().map(|range| ErrorPosition {
      location: CodeLocation::Source(range),
      message: None,
    })
  }
}

/// A key is present multiple times in the pattern of a rule
#[derive(Debug)]
struct ArityMismatch {
  locations: HashSet<(SourceRange, PHClass)>,
  name: Tok<String>,
}
impl ArityMismatch {
  #[must_use]
  pub fn new(rule: &ProjRule, name: Tok<String>) -> Self {
    let mut locations = HashSet::new();
    for expr in rule.template.iter() {
      expr.search_all(&mut |e| {
        if let Clause::Placeh(ph) = &e.value {
          if ph.name == name {
            locations.insert((e.range.clone(), ph.class));
          }
        }
        None::<()>
      });
    }
    Self { locations, name }
  }
}
impl ProjectError for ArityMismatch {
  const DESCRIPTION: &'static str =
    "A key appears with different arities in a rule";
  fn message(&self) -> String {
    format!(
      "The key {} appears multiple times with different arities in this rule",
      self.name
    )
  }
  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> {
    (self.locations.iter()).cloned().map(|(location, class)| ErrorPosition {
      location: CodeLocation::Source(location),
      message: Some(
        "This instance represents ".to_string()
          + match class {
            PHClass::Scalar => "one clause",
            PHClass::Name => "one name",
            PHClass::Vec { nonzero: true, .. } => "one or more clauses",
            PHClass::Vec { nonzero: false, .. } => "any number of clauses",
          },
      ),
    })
  }
}

/// Two vectorial placeholders appear next to each other
#[derive(Debug)]
struct VecNeighbors {
  locations: HashSet<SourceRange>,
  n1: Tok<String>,
  n2: Tok<String>,
}
impl VecNeighbors {
  #[must_use]
  pub fn new(rule: &ProjRule, n1: Tok<String>, n2: Tok<String>) -> Self {
    let mut locations = HashSet::new();
    search_all_slcs(&rule.template[..], &mut |ev| {
      for pair in ev.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let a_vec = matches!(&a.value, Clause::Placeh(
          Placeholder{ class: PHClass::Vec { .. }, name }
        ) if name == &n1);
        let b_vec = matches!(&b.value, Clause::Placeh(
          Placeholder{ class: PHClass::Vec { .. }, name }
        ) if name == &n2);
        if a_vec && b_vec {
          locations.insert(a.range.clone());
          locations.insert(b.range.clone());
        }
      }
      None::<()>
    });
    Self { locations, n1, n2 }
  }
}
impl ProjectError for VecNeighbors {
  const DESCRIPTION: &'static str =
    "Two vectorial placeholders appear next to each other";
  fn message(&self) -> String {
    format!(
      "The keys {} and {} appear next to each other with a vectorial arity",
      self.n1, self.n2
    )
  }
  fn positions(&self) -> impl IntoIterator<Item = ErrorPosition> {
    (self.locations.iter()).cloned().map(|location| ErrorPosition {
      location: CodeLocation::Source(location),
      message: None,
    })
  }
}
