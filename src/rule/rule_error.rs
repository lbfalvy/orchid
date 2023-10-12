use std::fmt::{self, Display};
use std::rc::Rc;

use hashbrown::HashSet;

use crate::ast::{self, search_all_slcs, PHClass, Placeholder, Rule};
use crate::error::{ErrorPosition, ProjectError};
#[allow(unused)] // for doc
use crate::foreign::ExternFn;
use crate::interner::Tok;
use crate::utils::BoxedIter;
use crate::{Location, Sym};

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
  /// Found an [ExternFn] in the pattern. This is a really unlikely mistake
  /// caused only by rogue systems.
  ExternFn,
}
impl RuleError {
  /// Convert into a unified error trait object shared by all Orchid errors
  #[must_use]
  pub fn to_project_error(self, rule: &Rule<Sym>) -> Rc<dyn ProjectError> {
    match self {
      Self::Missing(name) => Missing::new(rule, name).rc(),
      Self::Multiple(name) => Multiple::new(rule, name).rc(),
      Self::ArityMismatch(name) => ArityMismatch::new(rule, name).rc(),
      Self::VecNeighbors(n1, n2) => VecNeighbors::new(rule, n1, n2).rc(),
      Self::ExternFn => ExternFnInPattern(rule.clone()).rc(),
    }
  }
}

impl Display for RuleError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::ExternFn => write!(f, "Found an ExternFn in the pattern"),
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
pub struct Missing {
  locations: HashSet<Location>,
  name: Tok<String>,
}
impl Missing {
  #[must_use]
  pub fn new(rule: &ast::Rule<Sym>, name: Tok<String>) -> Self {
    let mut locations = HashSet::new();
    for expr in rule.template.iter() {
      expr.search_all(&mut |e| {
        if let ast::Clause::Placeh(ph) = &e.value {
          if ph.name == name {
            locations.insert(e.location.clone());
          }
        }
        None::<()>
      });
    }
    Self { locations, name }
  }
}
impl ProjectError for Missing {
  fn description(&self) -> &str {
    "A key appears in the template but not the pattern of a rule"
  }
  fn message(&self) -> String {
    format!(
      "The key {} appears in the template but not the pattern of this rule",
      self.name
    )
  }
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    Box::new(
      (self.locations.iter())
        .cloned()
        .map(|location| ErrorPosition { location, message: None }),
    )
  }
}

/// A key is present multiple times in the pattern of a rule
#[derive(Debug)]
pub struct Multiple {
  locations: HashSet<Location>,
  name: Tok<String>,
}
impl Multiple {
  #[must_use]
  pub fn new(rule: &ast::Rule<Sym>, name: Tok<String>) -> Self {
    let mut locations = HashSet::new();
    for expr in rule.template.iter() {
      expr.search_all(&mut |e| {
        if let ast::Clause::Placeh(ph) = &e.value {
          if ph.name == name {
            locations.insert(e.location.clone());
          }
        }
        None::<()>
      });
    }
    Self { locations, name }
  }
}
impl ProjectError for Multiple {
  fn description(&self) -> &str {
    "A key appears multiple times in the pattern of a rule"
  }
  fn message(&self) -> String {
    format!("The key {} appears multiple times in this pattern", self.name)
  }
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    Box::new(
      (self.locations.iter())
        .cloned()
        .map(|location| ErrorPosition { location, message: None }),
    )
  }
}

/// A key is present multiple times in the pattern of a rule
#[derive(Debug)]
pub struct ArityMismatch {
  locations: HashSet<(Location, ast::PHClass)>,
  name: Tok<String>,
}
impl ArityMismatch {
  #[must_use]
  pub fn new(rule: &ast::Rule<Sym>, name: Tok<String>) -> Self {
    let mut locations = HashSet::new();
    for expr in rule.template.iter() {
      expr.search_all(&mut |e| {
        if let ast::Clause::Placeh(ph) = &e.value {
          if ph.name == name {
            locations.insert((e.location.clone(), ph.class));
          }
        }
        None::<()>
      });
    }
    Self { locations, name }
  }
}
impl ProjectError for ArityMismatch {
  fn description(&self) -> &str {
    "A key appears with different arities in a rule"
  }
  fn message(&self) -> String {
    format!(
      "The key {} appears multiple times with different arities in this rule",
      self.name
    )
  }
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    Box::new((self.locations.iter()).cloned().map(|(location, class)| {
      ErrorPosition {
        location,
        message: Some(
          "This instance represents ".to_string()
            + match class {
              ast::PHClass::Scalar => "one clause",
              ast::PHClass::Vec { nonzero: true, .. } => "one or more clauses",
              ast::PHClass::Vec { nonzero: false, .. } =>
                "any number of clauses",
            },
        ),
      }
    }))
  }
}

/// Two vectorial placeholders appear next to each other
#[derive(Debug)]
pub struct VecNeighbors {
  locations: HashSet<Location>,
  n1: Tok<String>,
  n2: Tok<String>,
}
impl VecNeighbors {
  #[must_use]
  pub fn new(rule: &ast::Rule<Sym>, n1: Tok<String>, n2: Tok<String>) -> Self {
    let mut locations = HashSet::new();
    search_all_slcs(&rule.template[..], &mut |ev| {
      for pair in ev.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let a_vec = matches!(&a.value, ast::Clause::Placeh(
          Placeholder{ class: PHClass::Vec { .. }, name }
        ) if name == &n1);
        let b_vec = matches!(&b.value, ast::Clause::Placeh(
          Placeholder{ class: PHClass::Vec { .. }, name }
        ) if name == &n2);
        if a_vec && b_vec {
          locations.insert(a.location.clone());
          locations.insert(b.location.clone());
        }
      }
      None::<()>
    });
    Self { locations, n1, n2 }
  }
}
impl ProjectError for VecNeighbors {
  fn description(&self) -> &str {
    "Two vectorial placeholders appear next to each other"
  }
  fn message(&self) -> String {
    format!(
      "The keys {} and {} appear next to each other with a vectorial arity",
      self.n1, self.n2
    )
  }
  fn positions(&self) -> BoxedIter<ErrorPosition> {
    Box::new(
      (self.locations.iter())
        .cloned()
        .map(|location| ErrorPosition { location, message: None }),
    )
  }
}

/// Not referencing by location because it's most likely unknown
#[derive(Debug)]
pub struct ExternFnInPattern(ast::Rule<Sym>);
impl ProjectError for ExternFnInPattern {
  fn description(&self) -> &str {
    "Found an ExternFn in a pattern. Unlikely error caused by a system"
  }
  fn message(&self) -> String {
    format!("Found ExternFn in pattern {}", self.0)
  }
}
