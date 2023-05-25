use std::hash::Hash;
use std::rc::Rc;

use itertools::Itertools;
use ordered_float::NotNan;

use super::location::Location;
use super::primitive::Primitive;
use crate::interner::{InternedDisplay, Interner, Sym, Tok};
use crate::utils::Substack;

/// An S-expression with a type
#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
  pub value: Clause,
  pub location: Location,
}

impl Expr {
  pub fn into_clause(self) -> Clause {
    self.value
  }

  pub fn visit_names(&self, binds: Substack<Sym>, cb: &mut impl FnMut(Sym)) {
    let Expr { value, .. } = self;
    value.visit_names(binds, cb);
  }

  /// Process all names with the given mapper.
  /// Return a new object if anything was processed
  pub fn map_names(&self, pred: &impl Fn(Sym) -> Option<Sym>) -> Option<Self> {
    Some(Self {
      value: self.value.map_names(pred)?,
      location: self.location.clone(),
    })
  }

  /// Add the specified prefix to every Name
  pub fn prefix(
    &self,
    prefix: Sym,
    i: &Interner,
    except: &impl Fn(Tok<String>) -> bool,
  ) -> Self {
    Self {
      value: self.value.prefix(prefix, i, except),
      location: self.location.clone(),
    }
  }
}

impl InternedDisplay for Expr {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PHClass {
  Vec { nonzero: bool, prio: u64 },
  Scalar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Placeholder {
  pub name: Tok<String>,
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
pub enum Clause {
  P(Primitive),
  /// A c-style name or an operator, eg. `+`, `i`, `foo::bar`
  Name(Sym),
  /// A parenthesized exmrc_empty_slice()pression
  /// eg. `(print out "hello")`, `[1, 2, 3]`, `{Some(t) => t}`
  S(char, Rc<Vec<Expr>>),
  /// A function expression, eg. `\x. x + 1`
  Lambda(Rc<Expr>, Rc<Vec<Expr>>),
  /// A placeholder for macros, eg. `$name`, `...$body`, `...$lhs:1`
  Placeh(Placeholder),
}

impl Clause {
  /// Extract the expressions from an auto, lambda or S
  pub fn body(&self) -> Option<Rc<Vec<Expr>>> {
    match self {
      Self::Lambda(_, body) | Self::S(_, body) => Some(body.clone()),
      _ => None,
    }
  }

  /// Convert with identical meaning
  pub fn into_expr(self) -> Expr {
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
  pub fn from_exprs(exprs: &[Expr]) -> Option<Clause> {
    if exprs.is_empty() {
      None
    } else if exprs.len() == 1 {
      Some(exprs[0].clone().into_clause())
    } else {
      Some(Self::S('(', Rc::new(exprs.to_vec())))
    }
  }
  /// Convert with identical meaning
  pub fn from_exprv(exprv: &Rc<Vec<Expr>>) -> Option<Clause> {
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
  pub fn visit_names(&self, binds: Substack<Sym>, cb: &mut impl FnMut(Sym)) {
    match self {
      Clause::Lambda(arg, body) => {
        arg.visit_names(binds, cb);
        let new_binds = if let Clause::Name(n) = arg.value {
          binds.push(n)
        } else {
          binds
        };
        for x in body.iter() {
          x.visit_names(new_binds, cb)
        }
      },
      Clause::S(_, body) =>
        for x in body.iter() {
          x.visit_names(binds, cb)
        },
      Clause::Name(name) =>
        if binds.iter().all(|x| x != name) {
          cb(*name)
        },
      _ => (),
    }
  }

  /// Process all names with the given mapper.
  /// Return a new object if anything was processed
  pub fn map_names(&self, pred: &impl Fn(Sym) -> Option<Sym>) -> Option<Self> {
    match self {
      Clause::P(_) | Clause::Placeh(_) => None,
      Clause::Name(name) => pred(*name).map(Clause::Name),
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
        if any_some {
          Some(Clause::S(*c, Rc::new(new_body)))
        } else {
          None
        }
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

  /// Add the specified prefix to every Name
  pub fn prefix(
    &self,
    prefix: Sym,
    i: &Interner,
    except: &impl Fn(Tok<String>) -> bool,
  ) -> Self {
    self
      .map_names(&|name| {
        let old = i.r(name);
        if except(old[0]) {
          return None;
        }
        let mut new = i.r(prefix).clone();
        new.extend_from_slice(old);
        Some(i.i(&new))
      })
      .unwrap_or_else(|| self.clone())
  }
}

fn fmt_expr_seq<'a>(
  it: &mut impl Iterator<Item = &'a Expr>,
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

pub fn fmt_name(
  name: Sym,
  f: &mut std::fmt::Formatter,
  i: &Interner,
) -> std::fmt::Result {
  let strings = i.r(name).iter().map(|t| i.r(*t).as_str());
  for el in itertools::intersperse(strings, "::") {
    write!(f, "{}", el)?
  }
  Ok(())
}

impl InternedDisplay for Clause {
  fn fmt_i(
    &self,
    f: &mut std::fmt::Formatter<'_>,
    i: &Interner,
  ) -> std::fmt::Result {
    match self {
      Self::P(p) => write!(f, "{:?}", p),
      Self::Name(name) => fmt_name(*name, f, i),
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
pub struct Rule {
  pub source: Rc<Vec<Expr>>,
  pub prio: NotNan<f64>,
  pub target: Rc<Vec<Expr>>,
}

impl Rule {
  pub fn collect_single_names(&self, i: &Interner) -> Vec<Tok<String>> {
    let mut names = Vec::new();
    for e in self.source.iter() {
      e.visit_names(Substack::Bottom, &mut |tok| {
        let ns_name = i.r(tok);
        let (name, excess) =
          ns_name.split_first().expect("Namespaced name must not be empty");
        if !excess.is_empty() {
          return;
        }
        names.push(*name)
      });
    }
    names
  }

  pub fn prefix(
    &self,
    prefix: Sym,
    i: &Interner,
    except: &impl Fn(Tok<String>) -> bool,
  ) -> Self {
    Self {
      prio: self.prio,
      source: Rc::new(
        self.source.iter().map(|e| e.prefix(prefix, i, except)).collect(),
      ),
      target: Rc::new(
        self.target.iter().map(|e| e.prefix(prefix, i, except)).collect(),
      ),
    }
  }
}

impl InternedDisplay for Rule {
  fn fmt_i(
    &self,
    f: &mut std::fmt::Formatter<'_>,
    i: &Interner,
  ) -> std::fmt::Result {
    for e in self.source.iter() {
      e.fmt_i(f, i)?;
      write!(f, " ")?;
    }
    write!(f, "={}=>", self.prio)?;
    for e in self.target.iter() {
      write!(f, " ")?;
      e.fmt_i(f, i)?;
    }
    Ok(())
  }
}

/// A named constant
#[derive(Debug, Clone, PartialEq)]
pub struct Constant {
  pub name: Tok<String>,
  pub value: Expr,
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
