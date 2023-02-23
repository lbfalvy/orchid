use mappable_rc::Mrc;
use crate::executor::apply_lambda;
use crate::foreign::{Atom, ExternFn};
use crate::utils::{to_mrc_slice, one_mrc_slice};
use crate::utils::string_from_charset;

use super::get_name::get_name;
use super::{Literal, ast_to_typed, get_name};
use super::ast;

use std::fmt::{Debug, Write};

/// Indicates whether either side needs to be wrapped. Syntax whose end is ambiguous on that side
/// must use parentheses, or forward the flag
#[derive(PartialEq, Eq, Clone, Copy)]
struct Wrap(bool, bool);

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct Expr(pub Clause, pub Vec<Clause>);
impl Expr {
  fn deep_fmt(&self, f: &mut std::fmt::Formatter<'_>, tr: Wrap) -> std::fmt::Result {
    let Expr(val, typ) = self;
    if typ.len() > 0 {
      val.deep_fmt(f, Wrap(true, true))?;
      for typterm in typ {
        f.write_char(':')?;
        typterm.deep_fmt(f, Wrap(true, true))?;
      }
    } else {
      val.deep_fmt(f, tr)?;
    }
    Ok(())
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
  Apply(Box<Expr>, Box<Expr>),
  Lambda(u64, Box<[Clause]>, Box<Expr>),
  Auto(u64, Box<[Clause]>, Box<Expr>),
  LambdaArg(u64), AutoArg(u64),
  ExternFn(ExternFn),
  Atom(Atom)
}

const ARGNAME_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz";

fn parametric_fmt(
  f: &mut std::fmt::Formatter<'_>,
  prefix: &str, argtyp: &[Clause], body: &Expr, uid: u64, wrap_right: bool
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
      Self::Lambda(uid, argtyp, body) => parametric_fmt(f, "\\", argtyp, body, *uid, wr),
      Self::Auto(uid, argtyp, body) => parametric_fmt(f, "@", argtyp, body, *uid, wr),
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
  pub fn wrap(self) -> Box<Expr> { Box::new(Expr(self, vec![])) }
  pub fn wrap_t(self, t: Clause) -> Box<Expr> { Box::new(Expr(self, vec![t])) }
}

impl Clone for Clause {
  fn clone(&self) -> Self {
    match self {
      Clause::Auto(uid, t, b) => {
        let new_id = get_name();
        let new_body = apply_lambda(*uid, Clause::AutoArg(new_id).wrap(), b.clone());
        Clause::Auto(new_id, t.clone(), new_body)
      },
      Clause::Lambda(uid, t, b) => {
        let new_id = get_name();
        let new_body = apply_lambda(*uid, Clause::LambdaArg(new_id).wrap(), b.clone());
        Clause::Lambda(new_id, t.clone(), new_body)
      },
      Clause::Literal(l) => Clause::Literal(l.clone()),
      Clause::ExternFn(nc) => Clause::ExternFn(nc.clone()),
      Clause::Atom(a) => Clause::Atom(a.clone()),
      Clause::Apply(f, x) => Clause::Apply(Box::clone(&f), x.clone()),
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

pub fn is_used_clause(id: u64, is_auto: bool, clause: &Clause) -> bool {
  match clause {
    Clause::Atom(_) | Clause::ExternFn(_) | Clause::Literal(_) => false,
    Clause::AutoArg(x) => is_auto && *x == id,
    Clause::LambdaArg(x) => !is_auto && *x == id,
    Clause::Apply(f, x) => is_used_expr(id, is_auto, &f) || is_used_expr(id, is_auto, &x),
    Clause::Auto(n, t, b) => {
      assert!(*n != id, "Shadowing should have been eliminated");
      if is_auto && t.iter().any(|c| is_used_clause(id, is_auto, c)) {return true};
      is_used_expr(id, is_auto, b)
    }
    Clause::Lambda(n, t, b) => {
      assert!(*n != id, "Shadowing should have been eliminated");
      if is_auto && t.iter().any(|c| is_used_clause(id, is_auto, c)) {return true};
      is_used_expr(id, is_auto, b)
    }
  }
}

pub fn is_used_expr(id: u64, is_auto: bool, Expr(val, typ): &Expr) -> bool {
  if is_auto && typ.iter().any(|c| is_used_clause(id, is_auto, c)) {return true};
  is_used_clause(id, is_auto, val)
}