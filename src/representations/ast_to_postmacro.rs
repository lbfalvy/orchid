use std::rc::Rc;

use super::location::Location;
use super::{ast, postmacro};
use crate::error::ProjectError;
use crate::utils::Substack;
use crate::Sym;

#[derive(Debug, Clone)]
pub enum ErrorKind {
  /// `()` as a clause is meaningless in lambda calculus
  EmptyS,
  /// Only `(...)` may be converted to typed lambdas. `[...]` and `{...}`
  /// left in the code are signs of incomplete macro execution
  BadGroup(char),
  /// Placeholders shouldn't even occur in the code during macro
  /// execution. Something is clearly terribly wrong
  Placeholder,
  /// Arguments can only be a single [ast::Clause::Name]
  InvalidArg,
}

#[derive(Debug, Clone)]
pub struct Error {
  pub location: Location,
  pub kind: ErrorKind,
}
impl Error {
  pub fn new(kind: ErrorKind, location: &Location) -> Self {
    Self { location: location.clone(), kind }
  }
}
impl ProjectError for Error {
  fn description(&self) -> &str {
    match self.kind {
      ErrorKind::BadGroup(_) =>
        "Only `(...)` may be converted to postmacro. `[...]` and `{...}` left \
         in the code are signs of incomplete macro execution",
      ErrorKind::EmptyS => "`()` as a clause is meaningless in lambda calculus",
      ErrorKind::InvalidArg => "Argument names can only be Name nodes",
      ErrorKind::Placeholder =>
        "Placeholders shouldn't even appear in the code during macro \
         execution,this is likely a compiler bug",
    }
  }

  fn message(&self) -> String {
    match self.kind {
      ErrorKind::BadGroup(char) => format!("{} block found in the code", char),
      _ => self.description().to_string(),
    }
  }
  fn one_position(&self) -> Location {
    self.location.clone()
  }
}

/// Try to convert an expression from AST format to typed lambda
pub fn expr(expr: &ast::Expr<Sym>) -> Result<postmacro::Expr, Error> {
  expr_rec(expr, Context::new())
}

#[derive(Clone)]
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
  location: &'a Location,
  v: &'a [ast::Expr<Sym>],
  ctx: Context<'a>,
) -> Result<postmacro::Expr, Error> {
  let (last, rest) =
    (v.split_last()).ok_or_else(|| Error::new(ErrorKind::EmptyS, location))?;
  if rest.is_empty() {
    return expr_rec(&v[0], ctx);
  }
  let f = exprv_rec(location, rest, ctx.clone())?;
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
      return Err(Error::new(ErrorKind::BadGroup(*paren), location));
    }
    let expr = exprv_rec(location, body.as_ref(), ctx)?;
    Ok(postmacro::Expr { value: expr.value, location: location.clone() })
  } else {
    let value = match value {
      ast::Clause::P(p) => postmacro::Clause::P(p.clone()),
      ast::Clause::Lambda(arg, b) => {
        let name = match &arg[..] {
          [ast::Expr { value: ast::Clause::Name(name), .. }] => name,
          [ast::Expr { value: ast::Clause::Placeh { .. }, .. }] =>
            return Err(Error::new(ErrorKind::Placeholder, location)),
          _ => return Err(Error::new(ErrorKind::InvalidArg, location)),
        };
        let body_ctx = ctx.w_name(name.clone());
        let body = exprv_rec(location, b.as_ref(), body_ctx)?;
        postmacro::Clause::Lambda(Rc::new(body))
      },
      ast::Clause::Name(name) => {
        let lvl_opt = (ctx.names.iter())
          .enumerate()
          .find(|(_, n)| *n == name)
          .map(|(lvl, _)| lvl);
        match lvl_opt {
          Some(lvl) => postmacro::Clause::LambdaArg(lvl),
          None => postmacro::Clause::Constant(name.clone()),
        }
      },
      ast::Clause::S(paren, entries) => {
        if *paren != '(' {
          return Err(Error::new(ErrorKind::BadGroup(*paren), location));
        }
        let expr = exprv_rec(location, entries.as_ref(), ctx)?;
        expr.value
      },
      ast::Clause::Placeh { .. } =>
        return Err(Error::new(ErrorKind::Placeholder, location)),
    };
    Ok(postmacro::Expr { value, location: location.clone() })
  }
}
