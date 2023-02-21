use mappable_rc::Mrc;
use crate::foreign::{Atom, ExternFn};
use crate::utils::{to_mrc_slice, one_mrc_slice};
use crate::utils::string_from_charset;

use super::{Literal, ast_to_typed};
use super::ast;

use std::fmt::{Debug, Write};

/// Indicates whether either side needs to be wrapped. Syntax whose end is ambiguous on that side
/// must use parentheses, or forward the flag
#[derive(PartialEq, Eq)]
struct Wrap(bool, bool);

#[derive(PartialEq, Eq, Hash)]
pub struct Expr(pub Clause, pub Mrc<[Clause]>);
impl Expr {
  fn deep_fmt(&self, f: &mut std::fmt::Formatter<'_>, tr: Wrap) -> std::fmt::Result {
    let Expr(val, typ) = self;
    if typ.len() > 0 {
      val.deep_fmt(f, Wrap(true, true))?;
      for typ in typ.as_ref() {
        f.write_char(':')?;
        typ.deep_fmt(f, Wrap(true, true))?;
      }
    } else {
      val.deep_fmt(f, tr)?;
    }
    Ok(())
  }
}

impl Clone for Expr {
  fn clone(&self) -> Self {
    Self(self.0.clone(), Mrc::clone(&self.1))
  }
}

impl Debug for Expr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.deep_fmt(f, Wrap(false, false))
  }
}

#[derive(PartialEq, Eq, Hash)]
pub enum Clause {
  Literal(Literal),
  Apply(Mrc<Expr>, Mrc<Expr>),
  Lambda(u64, Mrc<[Clause]>, Mrc<Expr>),
  Auto(u64, Mrc<[Clause]>, Mrc<Expr>),
  LambdaArg(u64), AutoArg(u64),
  ExternFn(ExternFn),
  Atom(Atom)
}

const ARGNAME_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz";

fn parametric_fmt(
  f: &mut std::fmt::Formatter<'_>,
  prefix: &str, argtyp: Mrc<[Clause]>, body: Mrc<Expr>, uid: u64, wrap_right: bool
) -> std::fmt::Result {
  if wrap_right { f.write_char('(')?; }
  f.write_str(prefix)?;
  f.write_str(&string_from_charset(uid, ARGNAME_CHARSET))?;
  for typ in argtyp.iter() {
    f.write_str(":")?;
    typ.deep_fmt(f, Wrap(false, false))?;
  }
  f.write_str(".")?;
  body.deep_fmt(f, Wrap(false, false))?;
  if wrap_right { f.write_char(')')?; }
  Ok(())
}

impl Clause {
  fn deep_fmt(&self, f: &mut std::fmt::Formatter<'_>, Wrap(wl, wr): Wrap)
  -> std::fmt::Result {
    match self {
      Self::Literal(arg0) => write!(f, "{arg0:?}"),
      Self::ExternFn(nc) => write!(f, "{nc:?}"),
      Self::Atom(a) => write!(f, "{a:?}"),
      Self::Lambda(uid, argtyp, body) => parametric_fmt(f,
        "\\", Mrc::clone(argtyp), Mrc::clone(body), *uid, wr
      ),
      Self::Auto(uid, argtyp, body) => parametric_fmt(f,
        "@", Mrc::clone(argtyp), Mrc::clone(body), *uid, wr
      ),
      Self::LambdaArg(uid) | Self::AutoArg(uid) => f.write_str(&
        string_from_charset(*uid, ARGNAME_CHARSET)
      ),
      Self::Apply(func, x) => {
        if wl { f.write_char('(')?; }
        func.deep_fmt(f, Wrap(false, true) )?;
        f.write_char(' ')?;
        x.deep_fmt(f, Wrap(true, wr && !wl) )?;
        if wl { f.write_char(')')?; }
        Ok(())
      }
    }
  }
  pub fn wrap(self) -> Mrc<Expr> { Mrc::new(Expr(self, to_mrc_slice(vec![]))) }
  pub fn wrap_t(self, t: Clause) -> Mrc<Expr> { Mrc::new(Expr(self, one_mrc_slice(t))) }
}

impl Clone for Clause {
  fn clone(&self) -> Self {
    match self {
      Clause::Auto(uid,t, b) => Clause::Auto(*uid, Mrc::clone(t), Mrc::clone(b)),
      Clause::Lambda(uid, t, b) => Clause::Lambda(*uid, Mrc::clone(t), Mrc::clone(b)),
      Clause::Literal(l) => Clause::Literal(l.clone()),
      Clause::ExternFn(nc) => Clause::ExternFn(nc.clone()),
      Clause::Atom(a) => Clause::Atom(a.clone()),
      Clause::Apply(f, x) => Clause::Apply(Mrc::clone(f), Mrc::clone(x)),
      Clause::LambdaArg(id) => Clause::LambdaArg(*id),
      Clause::AutoArg(id) => Clause::AutoArg(*id)
    }
  }
}

impl Debug for Clause {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.deep_fmt(f, Wrap(false, false))
  }
}

impl TryFrom<&ast::Expr> for Expr {
  type Error = ast_to_typed::Error;
  fn try_from(value: &ast::Expr) -> Result<Self, Self::Error> {
    ast_to_typed::expr(value)
  }
}

impl TryFrom<&ast::Clause> for Clause {
  type Error = ast_to_typed::Error;
  fn try_from(value: &ast::Clause) -> Result<Self, Self::Error> {
    ast_to_typed::clause(value)
  }
}

pub fn count_references(id: u64, clause: &Clause)