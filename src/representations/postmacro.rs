use std::fmt::{Debug, Write};
use std::rc::Rc;

use super::location::Location;
use crate::foreign::{Atom, ExFn};
use crate::utils::string_from_charset;
use crate::Sym;

/// Indicates whether either side needs to be wrapped. Syntax whose end is
/// ambiguous on that side must use parentheses, or forward the flag
#[derive(PartialEq, Eq, Clone, Copy)]
struct Wrap(bool, bool);

#[derive(Clone)]
pub struct Expr {
  pub value: Clause,
  pub location: Location,
}

impl Expr {
  fn deep_fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
    depth: usize,
    tr: Wrap,
  ) -> std::fmt::Result {
    let Expr { value, .. } = self;
    value.deep_fmt(f, depth, tr)?;
    Ok(())
  }
}

impl Debug for Expr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.deep_fmt(f, 0, Wrap(false, false))
  }
}

#[derive(Clone)]
pub enum Clause {
  Apply(Rc<Expr>, Rc<Expr>),
  Lambda(Rc<Expr>),
  Constant(Sym),
  LambdaArg(usize),
  /// An opaque function, eg. an effectful function employing CPS
  ExternFn(ExFn),
  /// An opaque non-callable value, eg. a file handle
  Atom(Atom),
}

const ARGNAME_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz";

fn parametric_fmt(
  f: &mut std::fmt::Formatter<'_>,
  depth: usize,
  prefix: &str,
  body: &Expr,
  wrap_right: bool,
) -> std::fmt::Result {
  if wrap_right {
    f.write_char('(')?;
  }
  f.write_str(prefix)?;
  f.write_str(&string_from_charset(depth as u64, ARGNAME_CHARSET))?;
  f.write_str(".")?;
  body.deep_fmt(f, depth + 1, Wrap(false, false))?;
  if wrap_right {
    f.write_char(')')?;
  }
  Ok(())
}

impl Clause {
  fn deep_fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
    depth: usize,
    Wrap(wl, wr): Wrap,
  ) -> std::fmt::Result {
    match self {
      Self::Atom(a) => write!(f, "{a:?}"),
      Self::ExternFn(fun) => write!(f, "{fun:?}"),
      Self::Lambda(body) => parametric_fmt(f, depth, "\\", body, wr),
      Self::LambdaArg(skip) => {
        let lambda_depth = (depth - skip - 1).try_into().unwrap();
        f.write_str(&string_from_charset(lambda_depth, ARGNAME_CHARSET))
      },
      Self::Apply(func, x) => {
        if wl {
          f.write_char('(')?;
        }
        func.deep_fmt(f, depth, Wrap(false, true))?;
        f.write_char(' ')?;
        x.deep_fmt(f, depth, Wrap(true, wr && !wl))?;
        if wl {
          f.write_char(')')?;
        }
        Ok(())
      },
      Self::Constant(token) => write!(f, "{:?}", token),
    }
  }
}

impl Debug for Clause {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.deep_fmt(f, 0, Wrap(false, false))
  }
}
