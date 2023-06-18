use std::fmt::Display;
use std::rc::Rc;

use super::location::Location;
use super::{ast, postmacro};
use crate::utils::Substack;
use crate::Sym;

#[derive(Clone)]
pub enum Error {
  /// `()` as a clause is meaningless in lambda calculus
  EmptyS,
  /// Only `(...)` may be converted to typed lambdas. `[...]` and `{...}`
  /// left in the code are signs of incomplete macro execution
  BadGroup(char),
  /// Placeholders shouldn't even occur in the code during macro
  /// execution. Something is clearly terribly wrong
  Placeholder,
  /// Arguments can only be [ast::Clause::Name]
  InvalidArg,
}

impl Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Error::EmptyS => {
        write!(f, "`()` as a clause is meaningless in lambda calculus")
      },
      Error::BadGroup(_) => write!(
        f,
        "Only `(...)` may be converted to typed lambdas. `[...]` and \
         `{{...}}` left in the code are signs of incomplete macro execution"
      ),
      Error::Placeholder => write!(
        f,
        "Placeholders shouldn't even appear in the code during macro \
         execution, this is likely a compiler bug"
      ),
      Error::InvalidArg => write!(f, "Arguments can only be Name nodes"),
    }
  }
}

/// Try to convert an expression from AST format to typed lambda
pub fn expr(expr: &ast::Expr<Sym>) -> Result<postmacro::Expr, Error> {
  expr_rec(expr, Context::new())
}

#[derive(Clone, Copy)]
struct Context<'a> {
  names: Substack<'a, Sym>,
}

impl<'a> Context<'a> {
  fn w_name<'b>(&'b self, name: Sym) -> Context<'b>
  where
    'a: 'b,
  {
    Context { names: self.names.push(name) }
  }

  fn new() -> Context<'static> {
    Context { names: Substack::Bottom }
  }
}

/// Process an expression sequence
fn exprv_rec<'a>(
  v: &'a [ast::Expr<Sym>],
  ctx: Context<'a>,
) -> Result<postmacro::Expr, Error> {
  let (last, rest) = v.split_last().ok_or(Error::EmptyS)?;
  if rest.is_empty() {
    return expr_rec(&v[0], ctx);
  }
  let f = exprv_rec(rest, ctx)?;
  let x = expr_rec(last, ctx)?;
  let value = postmacro::Clause::Apply(Rc::new(f), Rc::new(x));
  Ok(postmacro::Expr { value, location: Location::Unknown })
}

/// Process an expression
fn expr_rec<'a>(
  ast::Expr { value, location }: &'a ast::Expr<Sym>,
  ctx: Context<'a>,
) -> Result<postmacro::Expr, Error> {
  if let ast::Clause::S(paren, body) = value {
    if *paren != '(' {
      return Err(Error::BadGroup(*paren));
    }
    let expr = exprv_rec(body.as_ref(), ctx)?;
    Ok(postmacro::Expr { value: expr.value, location: location.clone() })
  } else {
    let value = clause_rec(value, ctx)?;
    Ok(postmacro::Expr { value, location: location.clone() })
  }
}

/// Process a clause
fn clause_rec<'a>(
  cls: &'a ast::Clause<Sym>,
  ctx: Context<'a>,
) -> Result<postmacro::Clause, Error> {
  match cls {
    ast::Clause::P(p) => Ok(postmacro::Clause::P(p.clone())),
    ast::Clause::Lambda(expr, b) => {
      let name = match expr.value {
        ast::Clause::Name(name) => name,
        ast::Clause::Placeh { .. } => return Err(Error::Placeholder),
        _ => return Err(Error::InvalidArg),
      };
      let body_ctx = ctx.w_name(name);
      let body = exprv_rec(b.as_ref(), body_ctx)?;
      Ok(postmacro::Clause::Lambda(Rc::new(body)))
    },
    ast::Clause::Name(name) => {
      let lvl_opt = ctx
        .names
        .iter()
        .enumerate()
        .find(|(_, n)| *n == name)
        .map(|(lvl, _)| lvl);
      Ok(match lvl_opt {
        Some(lvl) => postmacro::Clause::LambdaArg(lvl),
        None => postmacro::Clause::Constant(*name),
      })
    },
    ast::Clause::S(paren, entries) => {
      if *paren != '(' {
        return Err(Error::BadGroup(*paren));
      }
      let expr = exprv_rec(entries.as_ref(), ctx)?;
      Ok(expr.value)
    },
    ast::Clause::Placeh { .. } => Err(Error::Placeholder),
  }
}
