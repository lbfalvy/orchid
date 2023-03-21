use lasso::RodeoResolver;
use lasso::Spur;
use itertools::Itertools;
use ordered_float::NotNan;
use std::hash::Hash;
use std::rc::Rc;
use crate::utils::InternedDisplay;
use crate::utils::Stackframe;

use super::primitive::Primitive;

/// An S-expression with a type
#[derive(PartialEq, Eq, Hash)]
pub struct Expr(pub Clause, pub Rc<Vec<Clause>>);
impl Expr {
  pub fn into_clause(self) -> Clause {
    if self.1.len() == 0 { self.0 }
    else { Clause::S('(', Rc::new(vec![self])) }
  }

  pub fn visit_names<F>(&self,
    binds: Stackframe<Rc<Vec<Spur>>>,
    cb: &mut F
  ) where F: FnMut(Rc<Vec<Spur>>)  {
    let Expr(val, typ) = self;
    val.visit_names(binds.clone(), cb);
    for typ in typ.as_ref() {
      typ.visit_names(binds.clone(), cb);
    }
  }
}

impl Clone for Expr {
  fn clone(&self) -> Self {
    Self(self.0.clone(), self.1.clone())
  }
}

impl InternedDisplay for Expr {
  fn fmt(&self,
    f: &mut std::fmt::Formatter<'_>,
    rr: RodeoResolver
  ) -> std::fmt::Result {
    let Expr(val, typ) = self;
    val.fmt(f, rr)?;
    for typ in typ.as_ref() {
      write!(f, ":")?;
      typ.fmt(f, rr)?;
    }
    Ok(())
  }
}

/// An S-expression as read from a source file
#[derive(PartialEq, Eq, Hash, Clone)]
pub enum Clause {
  P(Primitive),
  /// A c-style name or an operator, eg. `+`, `i`, `foo::bar`
  Name(Rc<Vec<Spur>>),
  /// A parenthesized exmrc_empty_slice()pression
  /// eg. `(print out "hello")`, `[1, 2, 3]`, `{Some(t) => t}`
  S(char, Rc<Vec<Expr>>),
  /// An explicit expression associated with the leftmost, outermost
  /// [Clause::Auto], eg. `read @Uint`
  Explicit(Rc<Expr>),
  /// A function expression, eg. `\x. x + 1`
  Lambda(Rc<Clause>, Rc<Vec<Expr>>, Rc<Vec<Expr>>),
  /// A parameterized expression with type inference, eg. `@T. T -> T`
  Auto(Option<Rc<Clause>>, Rc<Vec<Expr>>, Rc<Vec<Expr>>),
  /// A placeholder for macros, eg. `$name`, `...$body`, `...$lhs:1` 
  Placeh{
    key: String,
    /// None => matches one token
    /// Some((prio, nonzero)) =>
    ///   prio is the sizing priority for the vectorial
    ///     (higher prio grows first)
    ///   nonzero is whether the vectorial matches 1..n or 0..n tokens
    vec: Option<(usize, bool)>
  },
}

impl Clause {
  pub fn body(&self) -> Option<Rc<Vec<Expr>>> {
    match self {
      Self::Auto(_, _, body) | 
      Self::Lambda(_, _, body) |
      Self::S(_, body) => Some(body.clone()),
      _ => None
    }
  }
  pub fn typ(&self) -> Option<Rc<Vec<Expr>>> {
    match self {
      Self::Auto(_, typ, _) | Self::Lambda(_, typ, _) => Some(typ.clone()),
      _ => None
    }
  }
  pub fn into_expr(self) -> Expr {
    if let Self::S('(', body) = &self {
      if body.len() == 1 { body[0].clone() }
      else { Expr(self, Rc::default()) }
    } else { Expr(self, Rc::default()) }
  }
  pub fn from_exprv(exprv: &[Expr]) -> Option<Clause> {
    if exprv.len() == 0 { None }
    else if exprv.len() == 1 { Some(exprv[0].clone().into_clause()) }
    else { Some(Self::S('(', Rc::new(exprv.to_vec()))) }
  }

  /// Recursively iterate through all "names" in an expression.
  /// It also finds a lot of things that aren't names, such as all
  /// bound parameters. Generally speaking, this is not a very
  /// sophisticated search.
  pub fn visit_names<F>(&self,
    binds: Stackframe<Rc<Vec<Spur>>>,
    cb: &mut F
  ) where F: FnMut(Rc<Vec<Spur>>) {
    match self {
      Clause::Auto(name, typ, body) => {
        for x in typ.iter() {
          x.visit_names(binds.clone(), cb)
        }
        let binds_dup = binds.clone();
        let new_binds = if let Some(rc) = name {
          if let Clause::Name(name) = rc.as_ref() {
            binds_dup.push(name.clone())
          } else { binds }
        } else { binds };
        for x in body.iter() {
          x.visit_names(new_binds.clone(), cb)
        }
      },
      Clause::Lambda(name, typ, body) => {
        for x in typ.iter() {
          x.visit_names(binds.clone(), cb)
        }
        for x in body.iter() {
          let new_binds = if let Clause::Name(name) = name.as_ref() {
            binds.push(name.clone())
          } else { binds };
          x.visit_names(new_binds, cb)
        }
      },
      Clause::S(_, body) => for x in body.iter() {
        x.visit_names(binds.clone(), cb)
      },
      Clause::Name(name) => {
        if binds.iter().all(|x| x != name) {
          cb(name.clone())
        }
      }
      _ => (),
    }
  }
}

fn fmt_expr_seq(
  it: &mut dyn Iterator<Item = &Expr>,
  f: &mut std::fmt::Formatter<'_>,
  rr: RodeoResolver
) -> std::fmt::Result {
  for item in Itertools::intersperse(it.map(Some), None) { match item {
    Some(expr) => expr.fmt(f, rr),
    None => f.write_str(" "),
  }? }
  Ok(())
}

pub fn fmt_name(
  name: &Rc<Vec<Spur>>, f: &mut std::fmt::Formatter, rr: RodeoResolver
) -> std::fmt::Result {
  for el in itertools::intersperse(
    name.iter().map(|s| rr.resolve(s)),
    "::"
  ) {
    write!(f, "{}", el)?
  }
  Ok(())
}

impl InternedDisplay for Clause {
  fn fmt(&self,
    f: &mut std::fmt::Formatter<'_>,
    rr: RodeoResolver
  ) -> std::fmt::Result {
    match self {
      Self::P(p) => write!(f, "{:?}", p),
      Self::Name(name) => fmt_name(name, f, rr),
      Self::S(del, items) => {
        f.write_str(&del.to_string())?;
        fmt_expr_seq(&mut items.iter(), f, rr)?;
        f.write_str(match del {
          '(' => ")", '[' => "]", '{' => "}",
          _ => "CLOSING_DELIM"
        })
      },
      Self::Lambda(name, argtyp, body) => {
        f.write_str("\\")?;
        name.fmt(f, rr)?;
        f.write_str(":")?;
        fmt_expr_seq(&mut argtyp.iter(), f, rr)?;
        f.write_str(".")?;
        fmt_expr_seq(&mut body.iter(), f, rr)
      },
      Self::Auto(name_opt, argtyp, body) => {
        f.write_str("@")?;
        if let Some(name) = name_opt { name.fmt(f, rr)? }
        f.write_str(":")?;
        fmt_expr_seq(&mut argtyp.iter(), f, rr)?;
        f.write_str(".")?;
        fmt_expr_seq(&mut body.iter(), f, rr)
      },
      Self::Placeh{key, vec: None} => write!(f, "${key}"),
      Self::Placeh{key, vec: Some((prio, true))} =>
        write!(f, "...${key}:{prio}"),
      Self::Placeh{key, vec: Some((prio, false))} =>
        write!(f, "..${key}:{prio}"),
      Self::Explicit(expr) => {
        write!(f, "@")?;
        expr.fmt(f, rr)
      }
    }
  }
}

/// A substitution rule as read from the source
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Rule {
  pub source: Rc<Vec<Expr>>,
  pub prio: NotNan<f64>,
  pub target: Rc<Vec<Expr>>
}

impl InternedDisplay for Rule {
  fn fmt(&self,
    f: &mut std::fmt::Formatter<'_>,
    rr: RodeoResolver
  ) -> std::fmt::Result {
    for e in self.source.iter() { e.fmt(f, rr)?; write!(f, " ")?; }
    write!(f, "={}=>", self.prio)?;
    for e in self.target.iter() { write!(f, " ")?; e.fmt(f, rr)?; }
    Ok(())
  }
}