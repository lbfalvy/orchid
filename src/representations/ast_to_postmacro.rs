use std::rc::Rc;

use super::location::Location;
use super::{ast, postmacro};
use crate::ast::PType;
use crate::error::ProjectError;
use crate::utils::substack::Substack;
use crate::utils::unwrap_or;
use crate::Sym;

#[derive(Debug, Clone)]
pub enum ErrorKind {
  /// `()` as a clause is meaningless in lambda calculus
  EmptyS,
  /// Only `(...)` may be converted to typed lambdas. `[...]` and `{...}`
  /// left in the code are signs of incomplete macro execution
  BadGroup(PType),
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
  pub symbol: Sym,
}
impl Error {
  #[must_use]
  pub fn new(kind: ErrorKind, location: &Location, symbol: Sym) -> Self {
    Self { location: location.clone(), kind, symbol }
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
    if let ErrorKind::BadGroup(t) = self.kind {
      let sym = self.symbol.extern_vec().join("::");
      return format!("{}{} block found in {sym}", t.l(), t.r());
    }
    format!(
      "in {}, {}",
      self.symbol.extern_vec().join("::"),
      self.description()
    )
  }
  fn one_position(&self) -> Location { self.location.clone() }
}

/// Try to convert an expression from AST format to typed lambda
pub fn expr(
  expr: &ast::Expr<Sym>,
  symbol: Sym,
) -> Result<postmacro::Expr, Error> {
  expr_rec(expr, Context::new(symbol))
}

#[derive(Clone)]
struct Context<'a> {
  names: Substack<'a, Sym>,
  symbol: Sym,
}

impl<'a> Context<'a> {
  #[must_use]
  fn w_name<'b>(&'b self, name: Sym) -> Context<'b>
  where
    'a: 'b,
  {
    Context { names: self.names.push(name), symbol: self.symbol.clone() }
  }
}
impl Context<'static> {
  #[must_use]
  fn new(symbol: Sym) -> Self { Self { names: Substack::Bottom, symbol } }
}

/// Process an expression sequence
fn exprv_rec<'a>(
  location: &'a Location,
  v: &'a [ast::Expr<Sym>],
  ctx: Context<'a>,
) -> Result<postmacro::Expr, Error> {
  let (last, rest) = unwrap_or! {v.split_last(); {
    return Err(Error::new(ErrorKind::EmptyS, location, ctx.symbol));
  }};
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
  match value {
    ast::Clause::S(PType::Par, body) =>
      return Ok(postmacro::Expr {
        value: exprv_rec(location, body.as_ref(), ctx)?.value,
        location: location.clone(),
      }),
    ast::Clause::S(paren, _) =>
      return Err(Error::new(ErrorKind::BadGroup(*paren), location, ctx.symbol)),
    _ => (),
  }
  let value = match value {
    ast::Clause::Atom(a) => postmacro::Clause::Atom(a.clone()),
    ast::Clause::ExternFn(fun) => postmacro::Clause::ExternFn(fun.clone()),
    ast::Clause::Lambda(arg, b) => {
      let name = match &arg[..] {
        [ast::Expr { value: ast::Clause::Name(name), .. }] => name,
        [ast::Expr { value: ast::Clause::Placeh { .. }, .. }] =>
          return Err(Error::new(ErrorKind::Placeholder, location, ctx.symbol)),
        _ =>
          return Err(Error::new(ErrorKind::InvalidArg, location, ctx.symbol)),
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
    ast::Clause::S(PType::Par, entries) =>
      exprv_rec(location, entries.as_ref(), ctx)?.value,
    ast::Clause::S(paren, _) =>
      return Err(Error::new(ErrorKind::BadGroup(*paren), location, ctx.symbol)),
    ast::Clause::Placeh { .. } =>
      return Err(Error::new(ErrorKind::Placeholder, location, ctx.symbol)),
  };
  Ok(postmacro::Expr { value, location: location.clone() })
}
