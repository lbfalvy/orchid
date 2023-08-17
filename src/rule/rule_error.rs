use std::fmt;
use std::rc::Rc;

use hashbrown::HashSet;

use crate::ast::{self, search_all_slcs, PHClass, Placeholder, Rule};
use crate::error::{ErrorPosition, ProjectError};
use crate::interner::{InternedDisplay, Interner, Tok};
use crate::utils::BoxedIter;
use crate::{Location, Sym};

/// Various reasons why a substitution rule may be invalid
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
  pub fn to_project_error(self, rule: &Rule<Sym>) -> Rc<dyn ProjectError> {
    match self {
      RuleError::Missing(name) => Missing::new(rule, name).rc(),
      RuleError::Multiple(name) => Multiple::new(rule, name).rc(),
      RuleError::ArityMismatch(name) => ArityMismatch::new(rule, name).rc(),
      RuleError::VecNeighbors(n1, n2) => VecNeighbors::new(rule, n1, n2).rc(),
    }
  }
}

impl InternedDisplay for RuleError {
  fn fmt_i(&self, f: &mut fmt::Formatter<'_>, i: &Interner) -> fmt::Result {
    match *self {
      Self::Missing(key) => {
        write!(f, "Key {:?} not in match pattern", i.r(key))
      },
      Self::ArityMismatch(key) => write!(
        f,
        "Key {:?} used inconsistently with and without ellipsis",
        i.r(key)
      ),
      Self::Multiple(key) => {
        write!(f, "Key {:?} appears multiple times in match pattern", i.r(key))
      },
      Self::VecNeighbors(left, right) => write!(
        f,
        "Keys {:?} and {:?} are two vectorials right next to each other",
        i.r(left),
        i.r(right)
      ),
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
  fn message(&self, i: &Interner) -> String {
    format!(
      "The key {} appears in the template but not the pattern of this rule",
      i.r(self.name)
    )
  }
  fn positions(&self, _i: &Interner) -> BoxedIter<ErrorPosition> {
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
  fn message(&self, i: &Interner) -> String {
    format!("The key {} appears multiple times in this pattern", i.r(self.name))
  }
  fn positions(&self, _i: &Interner) -> BoxedIter<ErrorPosition> {
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
  fn message(&self, i: &Interner) -> String {
    format!(
      "The key {} appears multiple times with different arities in this rule",
      i.r(self.name)
    )
  }
  fn positions(&self, _i: &Interner) -> BoxedIter<ErrorPosition> {
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
  pub fn new(rule: &ast::Rule<Sym>, n1: Tok<String>, n2: Tok<String>) -> Self {
    let mut locations = HashSet::new();
    search_all_slcs(&rule.template[..], &mut |ev| {
      for pair in ev.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let a_vec = matches!(a.value, ast::Clause::Placeh(
          Placeholder{ class: PHClass::Vec { .. }, name }
        ) if name == n1);
        let b_vec = matches!(b.value, ast::Clause::Placeh(
          Placeholder{ class: PHClass::Vec { .. }, name }
        ) if name == n2);
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
  fn message(&self, i: &Interner) -> String {
    format!(
      "The keys {} and {} appear next to each other with a vectorial arity",
      i.r(self.n1),
      i.r(self.n2)
    )
  }
  fn positions(&self, _i: &Interner) -> BoxedIter<ErrorPosition> {
    Box::new(
      (self.locations.iter())
        .cloned()
        .map(|location| ErrorPosition { location, message: None }),
    )
  }
}
