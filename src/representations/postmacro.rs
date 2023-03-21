use crate::utils::string_from_charset;

use super::primitive::Primitive;

use std::fmt::{Debug, Write};
use std::rc::Rc;

/// Indicates whether either side needs to be wrapped. Syntax whose end is ambiguous on that side
/// must use parentheses, or forward the flag
#[derive(PartialEq, Eq, Clone, Copy)]
struct Wrap(bool, bool);

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct Expr(pub Clause, pub Rc<Vec<Clause>>);
impl Expr {
  fn deep_fmt(&self, f: &mut std::fmt::Formatter<'_>, depth: usize, tr: Wrap) -> std::fmt::Result {
    let Expr(val, typ) = self;
    if typ.len() > 0 {
      val.deep_fmt(f, depth, Wrap(true, true))?;
      for typterm in typ.as_ref() {
        f.write_char(':')?;
        typterm.deep_fmt(f, depth, Wrap(true, true))?;
      }
    } else {
      val.deep_fmt(f, depth, tr)?;
    }
    Ok(())
  }
}

impl Debug for Expr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.deep_fmt(f, 0, Wrap(false, false))
  }
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub enum Clause {
  Apply(Rc<Expr>, Rc<Expr>),
  Explicit(Rc<Expr>, Rc<Expr>),
  Lambda(Rc<Vec<Clause>>, Rc<Expr>),
  Auto(Rc<Vec<Clause>>, Rc<Expr>),
  LambdaArg(usize),
  AutoArg(usize),
  P(Primitive),
}

const ARGNAME_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz";

fn parametric_fmt(
  f: &mut std::fmt::Formatter<'_>, depth: usize,
  prefix: &str, argtyp: &[Clause], body: &Expr, wrap_right: bool
) -> std::fmt::Result {
  if wrap_right { f.write_char('(')?; }
  f.write_str(prefix)?;
  f.write_str(&string_from_charset(depth as u64, ARGNAME_CHARSET))?;
  for typ in argtyp.iter() {
    f.write_str(":")?;
    typ.deep_fmt(f, depth, Wrap(false, false))?;
  }
  f.write_str(".")?;
  body.deep_fmt(f, depth + 1, Wrap(false, false))?;
  if wrap_right { f.write_char(')')?; }
  Ok(())
}

impl Clause {
  fn deep_fmt(&self, f: &mut std::fmt::Formatter<'_>, depth: usize, Wrap(wl, wr): Wrap)
  -> std::fmt::Result {
    match self {
      Self::P(p) => write!(f, "{p:?}"),
      Self::Lambda(argtyp, body) => parametric_fmt(f, depth, "\\", argtyp, body, wr),
      Self::Auto(argtyp, body) => parametric_fmt(f, depth, "@", argtyp, body, wr),
      Self::LambdaArg(skip) | Self::AutoArg(skip) => {
        let lambda_depth = (depth - skip - 1).try_into().unwrap();
        f.write_str(&string_from_charset(lambda_depth, ARGNAME_CHARSET))
      },
      Self::Apply(func, x) => {
        if wl { f.write_char('(')?; }
        func.deep_fmt(f, depth, Wrap(false, true))?;
        f.write_char(' ')?;
        x.deep_fmt(f, depth, Wrap(true, wr && !wl))?;
        if wl { f.write_char(')')?; }
        Ok(())
      }
      Self::Explicit(gen, t) => {
        if wl { f.write_char('(')?; }
        gen.deep_fmt(f, depth, Wrap(false, true))?;
        f.write_str(" @")?;
        t.deep_fmt(f, depth, Wrap(true, wr && !wl))?;
        if wl { f.write_char(')'); }
        Ok(())
      }
    }
  }
  pub fn wrap(self) -> Box<Expr> { Box::new(Expr(self, Rc::new(vec![]))) }
  pub fn wrap_t(self, t: Clause) -> Box<Expr> { Box::new(Expr(self, Rc::new(vec![t]))) }
}

impl Debug for Clause {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.deep_fmt(f, 0, Wrap(false, false))
  }
}