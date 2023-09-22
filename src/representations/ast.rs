//! Datastructures representing the units of macro execution
//!
//! These structures are produced by the pipeline, processed by the macro
//! executor, and then converted to other usable formats.

use std::fmt::Display;
use std::hash::Hash;
use std::rc::Rc;

use hashbrown::HashSet;
use itertools::Itertools;
use ordered_float::NotNan;

#[allow(unused)] // for doc
use super::interpreted;
use super::location::Location;
use super::namelike::{NameLike, VName};
use super::primitive::Primitive;
use crate::interner::Tok;
use crate::parse::print_nat16;
use crate::utils::rc_tools::map_rc;

/// A [Clause] with associated metadata
#[derive(Clone, Debug, PartialEq)]
pub struct Expr<N: NameLike> {
  /// The actual value
  pub value: Clause<N>,
  /// Information about the code that produced this value
  pub location: Location,
}

impl<N: NameLike> Expr<N> {
  /// Process all names with the given mapper.
  /// Return a new object if anything was processed
  #[must_use]
  pub fn map_names(&self, pred: &impl Fn(&N) -> Option<N>) -> Option<Self> {
    Some(Self {
      value: self.value.map_names(pred)?,
      location: self.location.clone(),
    })
  }

  /// Transform from one name system to another
  #[must_use]
  pub fn transform_names<O: NameLike>(self, pred: &impl Fn(N) -> O) -> Expr<O> {
    Expr { value: self.value.transform_names(pred), location: self.location }
  }

  /// Visit all expressions in the tree. The search can be exited early by
  /// returning [Some]
  ///
  /// See also [interpreted::ExprInst::search_all]
  pub fn search_all<T>(
    &self,
    f: &mut impl FnMut(&Self) -> Option<T>,
  ) -> Option<T> {
    f(self).or_else(|| self.value.search_all(f))
  }
}

impl<N: NameLike> AsRef<Location> for Expr<N> {
  fn as_ref(&self) -> &Location { &self.location }
}

/// Visit all expression sequences including this sequence itself. Otherwise
/// works exactly like [Expr::search_all_slcs]
pub fn search_all_slcs<N: NameLike, T>(
  this: &[Expr<N>],
  f: &mut impl FnMut(&[Expr<N>]) -> Option<T>,
) -> Option<T> {
  f(this).or_else(|| this.iter().find_map(|expr| expr.value.search_all_slcs(f)))
}

impl Expr<VName> {
  /// Add the specified prefix to every Name
  #[must_use]
  pub fn prefix(
    &self,
    prefix: &[Tok<String>],
    except: &impl Fn(&Tok<String>) -> bool,
  ) -> Self {
    Self {
      value: self.value.prefix(prefix, except),
      location: self.location.clone(),
    }
  }
}

impl<N: NameLike> Display for Expr<N> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.value.fmt(f)
  }
}

/// Various types of placeholders
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PHClass {
  /// Matches multiple tokens, lambdas or parenthesized groups
  Vec {
    /// If true, must match at least one clause
    nonzero: bool,
    /// Greediness in the allocation of tokens
    prio: u64,
  },
  /// Matches exactly one token, lambda or parenthesized group
  Scalar,
}

/// Properties of a placeholder that matches unknown tokens in macros
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Placeholder {
  /// Identifier to pair placeholders in the pattern and template
  pub name: Tok<String>,
  /// The nature of the token set matched by this placeholder
  pub class: PHClass,
}

impl Display for Placeholder {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let name = &self.name;
    match self.class {
      PHClass::Scalar => write!(f, "${name}"),
      PHClass::Vec { nonzero, prio } => {
        if nonzero { write!(f, "...") } else { write!(f, "..") }?;
        write!(f, "${name}:{prio}")
      },
    }
  }
}

/// An S-expression as read from a source file
#[derive(Debug, PartialEq, Clone)]
pub enum Clause<N: NameLike> {
  /// A primitive
  P(Primitive),
  /// A c-style name or an operator, eg. `+`, `i`, `foo::bar`
  Name(N),
  /// A parenthesized expression
  /// eg. `(print out "hello")`, `[1, 2, 3]`, `{Some(t) => t}`
  S(char, Rc<Vec<Expr<N>>>),
  /// A function expression, eg. `\x. x + 1`
  Lambda(Rc<Vec<Expr<N>>>, Rc<Vec<Expr<N>>>),
  /// A placeholder for macros, eg. `$name`, `...$body`, `...$lhs:1`
  Placeh(Placeholder),
}

impl<N: NameLike> Clause<N> {
  /// Extract the expressions from an auto, lambda or S
  #[must_use]
  pub fn body(&self) -> Option<Rc<Vec<Expr<N>>>> {
    match self {
      Self::Lambda(_, body) | Self::S(_, body) => Some(body.clone()),
      _ => None,
    }
  }

  /// Convert with identical meaning
  #[must_use]
  pub fn into_expr(self) -> Expr<N> {
    if let Self::S('(', body) = &self {
      if body.len() == 1 {
        body[0].clone()
      } else {
        Expr { value: self, location: Location::Unknown }
      }
    } else {
      Expr { value: self, location: Location::Unknown }
    }
  }

  /// Convert with identical meaning
  #[must_use]
  pub fn from_exprs(exprs: &[Expr<N>]) -> Option<Self> {
    if exprs.is_empty() {
      None
    } else if exprs.len() == 1 {
      Some(exprs[0].value.clone())
    } else {
      Some(Self::S('(', Rc::new(exprs.to_vec())))
    }
  }

  /// Convert with identical meaning
  #[must_use]
  pub fn from_exprv(exprv: &Rc<Vec<Expr<N>>>) -> Option<Clause<N>> {
    if exprv.len() < 2 {
      Self::from_exprs(exprv)
    } else {
      Some(Self::S('(', exprv.clone()))
    }
  }

  /// Collect all names that appear in this expression.
  /// NOTICE: this isn't the total set of unbound names, it's mostly useful to
  /// make weak statements for optimization.
  #[must_use]
  pub fn collect_names(&self) -> HashSet<N> {
    if let Self::Name(n) = self {
      return HashSet::from([n.clone()]);
    }
    let mut glossary = HashSet::new();
    let result = self.search_all(&mut |e| {
      if let Clause::Name(n) = &e.value {
        glossary.insert(n.clone());
      }
      None::<()>
    });
    assert!(result.is_none(), "Callback never returns Some, wtf???");
    glossary
  }

  /// Process all names with the given mapper.
  /// Return a new object if anything was processed
  #[must_use]
  pub fn map_names(&self, pred: &impl Fn(&N) -> Option<N>) -> Option<Self> {
    match self {
      Clause::P(_) | Clause::Placeh(_) => None,
      Clause::Name(name) => pred(name).map(Clause::Name),
      Clause::S(c, body) => {
        let mut any_some = false;
        let new_body = body
          .iter()
          .map(|e| {
            let val = e.map_names(pred);
            any_some |= val.is_some();
            val.unwrap_or_else(|| e.clone())
          })
          .collect();
        if any_some { Some(Clause::S(*c, Rc::new(new_body))) } else { None }
      },
      Clause::Lambda(arg, body) => {
        let mut any_some = false;
        let new_arg = (arg.iter())
          .map(|e| {
            let val = e.map_names(pred);
            any_some |= val.is_some();
            val.unwrap_or_else(|| e.clone())
          })
          .collect();
        let new_body = (body.iter())
          .map(|e| {
            let val = e.map_names(pred);
            any_some |= val.is_some();
            val.unwrap_or_else(|| e.clone())
          })
          .collect();
        if any_some {
          Some(Clause::Lambda(Rc::new(new_arg), Rc::new(new_body)))
        } else {
          None
        }
      },
    }
  }

  /// Transform from one name representation to another
  #[must_use]
  pub fn transform_names<O: NameLike>(
    self,
    pred: &impl Fn(N) -> O,
  ) -> Clause<O> {
    match self {
      Self::Name(n) => Clause::Name(pred(n)),
      Self::Placeh(p) => Clause::Placeh(p),
      Self::P(p) => Clause::P(p),
      Self::Lambda(n, b) => Clause::Lambda(
        map_rc(n, |n| n.into_iter().map(|e| e.transform_names(pred)).collect()),
        map_rc(b, |b| b.into_iter().map(|e| e.transform_names(pred)).collect()),
      ),
      Self::S(c, b) => Clause::S(
        c,
        map_rc(b, |b| b.into_iter().map(|e| e.transform_names(pred)).collect()),
      ),
    }
  }

  /// Pair of [Expr::search_all]
  pub fn search_all<T>(
    &self,
    f: &mut impl FnMut(&Expr<N>) -> Option<T>,
  ) -> Option<T> {
    match self {
      Clause::Lambda(arg, body) =>
        arg.iter().chain(body.iter()).find_map(|expr| expr.search_all(f)),
      Clause::Name(_) | Clause::P(_) | Clause::Placeh(_) => None,
      Clause::S(_, body) => body.iter().find_map(|expr| expr.search_all(f)),
    }
  }

  /// Pair of [Expr::search_all_slcs]
  pub fn search_all_slcs<T>(
    &self,
    f: &mut impl FnMut(&[Expr<N>]) -> Option<T>,
  ) -> Option<T> {
    match self {
      Clause::Lambda(arg, body) =>
        search_all_slcs(arg, f).or_else(|| search_all_slcs(body, f)),
      Clause::Name(_) | Clause::P(_) | Clause::Placeh(_) => None,
      Clause::S(_, body) => search_all_slcs(body, f),
    }
  }
}

impl Clause<VName> {
  /// Add the specified prefix to every Name
  #[must_use]
  pub fn prefix(
    &self,
    prefix: &[Tok<String>],
    except: &impl Fn(&Tok<String>) -> bool,
  ) -> Self {
    self
      .map_names(&|name| {
        if except(&name[0]) {
          return None;
        }
        let mut new = prefix.to_vec();
        new.extend_from_slice(name);
        Some(new)
      })
      .unwrap_or_else(|| self.clone())
  }
}

impl<N: NameLike> Display for Clause<N> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::P(p) => write!(f, "{:?}", p),
      Self::Name(name) => write!(f, "{}", name.to_strv().join("::")),
      Self::S(del, items) => {
        let body = items.iter().join(" ");
        let led = match del {
          '(' => ")",
          '[' => "]",
          '{' => "}",
          _ => "CLOSING_DELIM",
        };
        write!(f, "{del}{body}{led}")
      },
      Self::Lambda(arg, body) => {
        let args = arg.iter().join(" ");
        let bodys = body.iter().join(" ");
        write!(f, "\\{args}.{bodys}")
      },
      Self::Placeh(ph) => ph.fmt(f),
    }
  }
}

/// A substitution rule as read from the source
#[derive(Debug, Clone, PartialEq)]
pub struct Rule<N: NameLike> {
  /// Tree fragment in the source code that activates this rule
  pub pattern: Vec<Expr<N>>,
  /// Influences the order in which rules are checked
  pub prio: NotNan<f64>,
  /// Tree fragment generated by this rule
  pub template: Vec<Expr<N>>,
}

impl Rule<VName> {
  /// Namespace all tokens in the rule
  #[must_use]
  pub fn prefix(
    &self,
    prefix: &[Tok<String>],
    except: &impl Fn(&Tok<String>) -> bool,
  ) -> Self {
    Self {
      prio: self.prio,
      pattern: self.pattern.iter().map(|e| e.prefix(prefix, except)).collect(),
      template: (self.template.iter())
        .map(|e| e.prefix(prefix, except))
        .collect(),
    }
  }

  /// Return a list of all names that don't contain a namespace separator `::`.
  /// These are exported when the rule is exported
  #[must_use]
  pub fn collect_single_names(&self) -> VName {
    let mut names = Vec::new();
    for e in self.pattern.iter() {
      e.search_all(&mut |e| {
        if let Clause::Name(ns_name) = &e.value {
          if ns_name.len() == 1 {
            names.push(ns_name[0].clone())
          }
        }
        None::<()>
      });
    }
    names
  }
}

impl<N: NameLike> Display for Rule<N> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "rule {} ={}=> {}",
      self.pattern.iter().join(" "),
      print_nat16(self.prio),
      self.template.iter().join(" ")
    )
  }
}

/// A named constant
#[derive(Debug, Clone, PartialEq)]
pub struct Constant {
  /// Used to reference the constant
  pub name: Tok<String>,
  /// The constant value inserted where the name is found
  pub value: Expr<VName>,
}

impl Display for Constant {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "const {} := {}", *self.name, self.value)
  }
}
