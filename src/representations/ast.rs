//! Datastructures representing the units of macro execution
//!
//! These structures are produced by the pipeline, processed by the macro
//! executor, and then converted to other usable formats.

use std::hash::Hash;
use std::rc::Rc;

use itertools::Itertools;
use ordered_float::NotNan;

use super::location::Location;
use super::namelike::{NameLike, VName};
use super::primitive::Primitive;
use crate::interner::{InternedDisplay, Interner, Tok};
use crate::utils::{map_rc, Substack};

/// A [Clause] with associated metadata
#[derive(Clone, Debug, PartialEq)]
pub struct Expr<N: NameLike> {
  /// The actual value
  pub value: Clause<N>,
  /// Information about the code that produced this value
  pub location: Location,
}

impl<N: NameLike> Expr<N> {
  /// Obtain the contained clause
  pub fn into_clause(self) -> Clause<N> {
    self.value
  }

  /// Call the function on every name in this expression
  pub fn visit_names(&self, binds: Substack<&N>, cb: &mut impl FnMut(&N)) {
    let Expr { value, .. } = self;
    value.visit_names(binds, cb);
  }

  /// Process all names with the given mapper.
  /// Return a new object if anything was processed
  pub fn map_names(&self, pred: &impl Fn(&N) -> Option<N>) -> Option<Self> {
    Some(Self {
      value: self.value.map_names(pred)?,
      location: self.location.clone(),
    })
  }

  /// Transform from one name system to another
  pub fn transform_names<O: NameLike>(self, pred: &impl Fn(N) -> O) -> Expr<O> {
    Expr { value: self.value.transform_names(pred), location: self.location }
  }
}

impl Expr<VName> {
  /// Add the specified prefix to every Name
  pub fn prefix(
    &self,
    prefix: &[Tok<String>],
    except: &impl Fn(Tok<String>) -> bool,
  ) -> Self {
    Self {
      value: self.value.prefix(prefix, except),
      location: self.location.clone(),
    }
  }
}

impl<N: NameLike> InternedDisplay for Expr<N> {
  fn fmt_i(
    &self,
    f: &mut std::fmt::Formatter<'_>,
    i: &Interner,
  ) -> std::fmt::Result {
    let Expr { value, .. } = self;
    value.fmt_i(f, i)?;
    Ok(())
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Placeholder {
  /// Identifier to pair placeholders in the pattern and template
  pub name: Tok<String>,
  /// The nature of the token set matched by this placeholder
  pub class: PHClass,
}

impl InternedDisplay for Placeholder {
  fn fmt_i(
    &self,
    f: &mut std::fmt::Formatter<'_>,
    i: &Interner,
  ) -> std::fmt::Result {
    let name = i.r(self.name);
    match self.class {
      PHClass::Scalar => write!(f, "${name}"),
      PHClass::Vec { nonzero, prio } =>
        if nonzero {
          write!(f, "...${name}:{prio}")
        } else {
          write!(f, "..${name}:{prio}")
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
  Lambda(Rc<Expr<N>>, Rc<Vec<Expr<N>>>),
  /// A placeholder for macros, eg. `$name`, `...$body`, `...$lhs:1`
  Placeh(Placeholder),
}

impl<N: NameLike> Clause<N> {
  /// Extract the expressions from an auto, lambda or S
  pub fn body(&self) -> Option<Rc<Vec<Expr<N>>>> {
    match self {
      Self::Lambda(_, body) | Self::S(_, body) => Some(body.clone()),
      _ => None,
    }
  }

  /// Convert with identical meaning
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
  pub fn from_exprs(exprs: &[Expr<N>]) -> Option<Self> {
    if exprs.is_empty() {
      None
    } else if exprs.len() == 1 {
      Some(exprs[0].clone().into_clause())
    } else {
      Some(Self::S('(', Rc::new(exprs.to_vec())))
    }
  }
  /// Convert with identical meaning
  pub fn from_exprv(exprv: &Rc<Vec<Expr<N>>>) -> Option<Clause<N>> {
    if exprv.len() < 2 {
      Self::from_exprs(exprv)
    } else {
      Some(Self::S('(', exprv.clone()))
    }
  }

  /// Recursively iterate through all "names" in an expression.
  /// It also finds a lot of things that aren't names, such as all
  /// bound parameters. Generally speaking, this is not a very
  /// sophisticated search.
  pub fn visit_names(&self, binds: Substack<&N>, cb: &mut impl FnMut(&N)) {
    match self {
      Clause::Lambda(arg, body) => {
        arg.visit_names(binds, cb);
        let new_binds =
          if let Clause::Name(n) = &arg.value { binds.push(n) } else { binds };
        for x in body.iter() {
          x.visit_names(new_binds, cb)
        }
      },
      Clause::S(_, body) =>
        for x in body.iter() {
          x.visit_names(binds, cb)
        },
      Clause::Name(name) =>
        if binds.iter().all(|x| x != &name) {
          cb(name)
        },
      _ => (),
    }
  }

  /// Process all names with the given mapper.
  /// Return a new object if anything was processed
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
        let new_arg = arg.map_names(pred);
        let mut any_some = new_arg.is_some();
        let new_body = body
          .iter()
          .map(|e| {
            let val = e.map_names(pred);
            any_some |= val.is_some();
            val.unwrap_or_else(|| e.clone())
          })
          .collect();
        if any_some {
          Some(Clause::Lambda(
            new_arg.map(Rc::new).unwrap_or_else(|| arg.clone()),
            Rc::new(new_body),
          ))
        } else {
          None
        }
      },
    }
  }

  /// Transform from one name representation to another
  pub fn transform_names<O: NameLike>(
    self,
    pred: &impl Fn(N) -> O,
  ) -> Clause<O> {
    match self {
      Self::Name(n) => Clause::Name(pred(n)),
      Self::Placeh(p) => Clause::Placeh(p),
      Self::P(p) => Clause::P(p),
      Self::Lambda(n, b) => Clause::Lambda(
        map_rc(n, |n| n.transform_names(pred)),
        map_rc(b, |b| b.into_iter().map(|e| e.transform_names(pred)).collect()),
      ),
      Self::S(c, b) => Clause::S(
        c,
        map_rc(b, |b| b.into_iter().map(|e| e.transform_names(pred)).collect()),
      ),
    }
  }
}

impl Clause<VName> {
  /// Add the specified prefix to every Name
  pub fn prefix(
    &self,
    prefix: &[Tok<String>],
    except: &impl Fn(Tok<String>) -> bool,
  ) -> Self {
    self
      .map_names(&|name| {
        if except(name[0]) {
          return None;
        }
        let mut new = prefix.to_vec();
        new.extend_from_slice(name);
        Some(new)
      })
      .unwrap_or_else(|| self.clone())
  }
}

fn fmt_expr_seq<'a, N: NameLike>(
  it: &mut impl Iterator<Item = &'a Expr<N>>,
  f: &mut std::fmt::Formatter<'_>,
  i: &Interner,
) -> std::fmt::Result {
  for item in Itertools::intersperse(it.map(Some), None) {
    match item {
      Some(expr) => expr.fmt_i(f, i),
      None => f.write_str(" "),
    }?
  }
  Ok(())
}

impl<N: NameLike> InternedDisplay for Clause<N> {
  fn fmt_i(
    &self,
    f: &mut std::fmt::Formatter<'_>,
    i: &Interner,
  ) -> std::fmt::Result {
    match self {
      Self::P(p) => write!(f, "{:?}", p),
      Self::Name(name) => write!(f, "{}", name.to_strv(i).join("::")),
      Self::S(del, items) => {
        f.write_str(&del.to_string())?;
        fmt_expr_seq(&mut items.iter(), f, i)?;
        f.write_str(match del {
          '(' => ")",
          '[' => "]",
          '{' => "}",
          _ => "CLOSING_DELIM",
        })
      },
      Self::Lambda(arg, body) => {
        f.write_str("\\")?;
        arg.fmt_i(f, i)?;
        f.write_str(".")?;
        fmt_expr_seq(&mut body.iter(), f, i)
      },
      Self::Placeh(ph) => ph.fmt_i(f, i),
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
  pub fn prefix(
    &self,
    prefix: &[Tok<String>],
    except: &impl Fn(Tok<String>) -> bool,
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
  pub fn collect_single_names(&self) -> Vec<Tok<String>> {
    let mut names = Vec::new();
    for e in self.pattern.iter() {
      e.visit_names(Substack::Bottom, &mut |ns_name| {
        if ns_name.len() == 1 {
          names.push(ns_name[0])
        }
      });
    }
    names
  }
}

impl<N: NameLike> InternedDisplay for Rule<N> {
  fn fmt_i(
    &self,
    f: &mut std::fmt::Formatter<'_>,
    i: &Interner,
  ) -> std::fmt::Result {
    for e in self.pattern.iter() {
      e.fmt_i(f, i)?;
      write!(f, " ")?;
    }
    write!(f, "={}=>", self.prio)?;
    for e in self.template.iter() {
      write!(f, " ")?;
      e.fmt_i(f, i)?;
    }
    Ok(())
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

impl InternedDisplay for Constant {
  fn fmt_i(
    &self,
    f: &mut std::fmt::Formatter<'_>,
    i: &Interner,
  ) -> std::fmt::Result {
    write!(f, "{} := ", i.r(self.name))?;
    self.value.fmt_i(f, i)
  }
}
